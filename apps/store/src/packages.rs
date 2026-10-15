use crate::{model::Installed, repositories};
use alpm::{Alpm, SigLevel};
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub fn provenance_path() -> PathBuf {
    Path::new(repositories::STATE).join("provenance.sqlite")
}

pub fn handle(cache: &Path, root: bool, refresh: bool) -> Result<Alpm> {
    let mut conf = repositories::system_config()?;
    conf.sig_level =
        vec!["PackageRequired".into(), "PackageTrustOnly".into(), "DatabaseOptional".into(), "DatabaseTrustOnly".into()];
    for repo in &mut conf.repos {
        repo.sig_level = conf.sig_level.clone();
    }
    // Commits must use pacman's real local database. Libalpm replaces a local/
    // symlink when writing entries, silently splitting installed package records.
    let path = if root { PathBuf::from("/var/lib/pacman") } else { cache.join("alpm") };
    if !root {
        fs::create_dir_all(path.join("sync"))?;
        let local = path.join("local");
        if !local.exists() {
            std::os::unix::fs::symlink("/var/lib/pacman/local", &local)?;
        }
        ensure!(
            local.is_symlink() && fs::read_link(&local)? == Path::new("/var/lib/pacman/local"),
            "Invalid Store catalog database link"
        );
        for r in &conf.repos {
            for suffix in ["db", "db.sig"] {
                let name = format!("{}.{suffix}", r.name);
                let dest = path.join("sync").join(&name);
                let origin = Path::new("/var/lib/pacman/sync").join(&name);
                if origin.exists() && (!dest.exists() || origin.metadata()?.modified()? > dest.metadata()?.modified()?) {
                    fs::copy(origin, dest)?;
                }
            }
        }
    }
    conf.db_path = path.to_string_lossy().into_owned();
    conf.repos.retain(|r| repositories::official(&r.name));
    // Pacmanconf still supplies hook paths, architecture, held files and ignored packages.
    let mut h = alpm_utils::alpm_with_conf(&conf)?;
    h.set_ignoregroups(conf.ignore_group.iter())?;
    h.set_question_cb((), |mut question, _| question.set_answer(false));
    h.set_default_siglevel(SigLevel::PACKAGE | SigLevel::DATABASE | SigLevel::DATABASE_OPTIONAL)?;
    if !root {
        h.set_logfile(cache.join("catalog.log").to_string_lossy().as_bytes())?;
        h.set_sandbox_user(None::<&str>)?;
    }
    for r in &conf.repos {
        h.syncdbs().iter().find(|d| d.name() == r.name).unwrap().is_valid().ok();
    }
    let original = repositories::system_config()?;
    let custom = repositories::custom()?;
    for r in original.repos.iter().filter(|r| !repositories::official(&r.name) && !custom.iter().any(|c| c.name == r.name)) {
        let db = h.register_syncdb_mut(r.name.as_str(), SigLevel::PACKAGE | SigLevel::DATABASE | SigLevel::DATABASE_OPTIONAL)?;
        db.set_servers(r.servers.iter())?;
    }
    for repo in custom.iter().filter(|r| r.enabled) {
        repositories::validate(repo)?;
        let db =
            h.register_syncdb_mut(repo.name.as_str(), SigLevel::PACKAGE | SigLevel::DATABASE | SigLevel::DATABASE_OPTIONAL)?;
        let servers: Vec<_> = repo.servers.iter().map(|s| s.replace("$arch", "x86_64").replace("$repo", &repo.name)).collect();
        db.set_servers(servers.iter())?;
    }
    if refresh {
        h.syncdbs_mut().update(false)?;
    }
    Ok(h)
}

pub fn stamp(p: &alpm::Package) -> String {
    format!("{}:{}:{}:{}", p.version(), p.install_date().unwrap_or(0), p.build_date(), p.packager().unwrap_or(""))
}

pub fn installed(h: &Alpm) -> BTreeMap<String, Installed> {
    h.localdb()
        .pkgs()
        .iter()
        .map(|p| (p.name().into(), Installed { name: p.name().into(), version: p.version().to_string(), stamp: stamp(p) }))
        .collect()
}

pub fn snapshot(h: &Alpm) -> String {
    let mut digest = Sha256::new();
    for p in installed(h).values() {
        digest.update(p.name.as_bytes());
        digest.update([0]);
        digest.update(p.stamp.as_bytes());
        digest.update([0]);
    }
    format!("{:x}", digest.finalize())
}

pub fn pkg<'a>(h: &'a Alpm, source: &str, name: &str) -> Result<&'a alpm::Package> {
    let db = h
        .syncdbs()
        .iter()
        .find(|d| d.name() == source)
        .ok_or_else(|| anyhow::anyhow!("Repository {source} is disabled or unavailable"))?;
    Ok(db.pkg(name)?)
}

pub fn preferred<'a>(h: &'a Alpm, name: &str) -> Option<&'a alpm::Package> {
    h.syncdbs().iter().find_map(|d| d.pkg(name).ok())
}

/// Recover records left by the old symlink-based writer, while holding pacman's
/// native transaction lock. Never overwrite an existing installed package record.
pub fn recover_legacy_local() -> Result<()> {
    let legacy = Path::new(repositories::STATE).join("alpm/local");
    if !legacy.is_dir() || legacy.is_symlink() {
        return Ok(());
    }
    for entry in fs::read_dir(&legacy)? {
        let entry = entry?;
        ensure!(entry.file_type()?.is_dir(), "Unexpected legacy package database entry");
        let name = entry.file_name();
        let target = Path::new("/var/lib/pacman/local").join(&name);
        ensure!(entry.path().join("desc").is_file() && entry.path().join("files").is_file(), "Incomplete legacy package record");
        ensure!(
            !target.exists() || fs::read_dir(&target)?.next().is_none(),
            "Cannot recover {}: an installed record already exists",
            name.to_string_lossy()
        );
        fs::rename(entry.path(), &target).context("Recovering the interrupted package database record")?;
    }
    Ok(())
}
