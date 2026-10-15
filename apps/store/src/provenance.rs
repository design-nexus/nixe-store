use crate::model::{Installed, Plan};
use anyhow::{Result, ensure};
use rusqlite::{Connection, params};
use std::{collections::BTreeMap, path::Path};

pub struct Provenance {
    db: Connection,
}

impl Provenance {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path)?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        db.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS packages(name TEXT PRIMARY KEY, source TEXT NOT NULL, stamp TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS journal(id INTEGER PRIMARY KEY CHECK(id=1), plan TEXT NOT NULL);",
        )?;
        Ok(Self { db })
    }
    pub fn sources(path: &Path) -> Result<BTreeMap<String, (String, String)>> {
        if !path.exists() {
            return Ok(BTreeMap::new());
        }
        let db = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        let mut stmt = db.prepare("SELECT name,source,stamp FROM packages")?;
        Ok(stmt.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?.collect::<std::result::Result<_, _>>()?)
    }
    pub fn assign(&mut self, assignments: &[(String, String, String)], installed: &BTreeMap<String, Installed>) -> Result<()> {
        let tx = self.db.transaction()?;
        for (name, source, stamp) in assignments {
            let pkg = installed.get(name).ok_or_else(|| anyhow::anyhow!("{name} is no longer installed"))?;
            ensure!(&pkg.stamp == stamp, "{name} changed since the source review; refresh and try again");
            tx.execute("INSERT OR REPLACE INTO packages VALUES (?1,?2,?3)", params![name, source, stamp])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn begin(&self, plan: &Plan) -> Result<()> {
        self.db.execute("INSERT OR REPLACE INTO journal VALUES (1,?1)", [serde_json::to_string(plan)?])?;
        Ok(())
    }
    pub fn reconcile(&mut self, installed: &BTreeMap<String, Installed>) -> Result<()> {
        let pending = self.db.query_row("SELECT plan FROM journal WHERE id=1", [], |r| r.get::<_, String>(0)).ok();
        let tx = self.db.transaction()?;
        if let Some(pending) = pending {
            let plan: Plan = serde_json::from_str(&pending)?;
            for change in &plan.changes {
                match installed.get(&change.name) {
                    Some(pkg) if change.new.as_deref() == Some(&pkg.version) => {
                        tx.execute(
                            "INSERT OR REPLACE INTO packages VALUES (?1,?2,?3)",
                            params![pkg.name, change.source, pkg.stamp],
                        )?;
                    }
                    None => {
                        tx.execute("DELETE FROM packages WHERE name=?1", [&change.name])?;
                    }
                    _ => {}
                }
            }
        }
        let rows: Vec<(String, String)> = {
            let mut s = tx.prepare("SELECT name,stamp FROM packages")?;
            s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?
        };
        for (name, stamp) in rows {
            if !installed.get(&name).is_some_and(|p| p.stamp == stamp) {
                tx.execute("DELETE FROM packages WHERE name=?1", [name])?;
            }
        }
        tx.execute("DELETE FROM journal", [])?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Change;
    #[test]
    fn reconcile_partial_commit_and_external_changes() {
        let dir = std::env::temp_dir().join(format!(
            "nixe-store-journal-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("state.sqlite");
        let mut db = Provenance::open(&path).unwrap();
        let old = Installed { name: "editor".into(), version: "1".into(), stamp: "1:10".into() };
        let mut actual = BTreeMap::from([("editor".into(), old)]);
        db.assign(&[("editor".into(), "extra".into(), "1:10".into())], &actual).unwrap();
        let plan = Plan {
            changes: vec![Change {
                name: "editor".into(),
                old: Some("1".into()),
                new: Some("2".into()),
                source: "extra".into(),
                bytes: 0,
            }],
            ..Default::default()
        };
        db.begin(&plan).unwrap();
        db.reconcile(&actual).unwrap();
        assert_eq!(Provenance::sources(&path).unwrap()["editor"].1, "1:10");
        db.begin(&plan).unwrap();
        actual.insert("editor".into(), Installed { name: "editor".into(), version: "2".into(), stamp: "2:20".into() });
        db.reconcile(&actual).unwrap();
        assert_eq!(Provenance::sources(&path).unwrap()["editor"], ("extra".into(), "2:20".into()));
        actual.get_mut("editor").unwrap().stamp = "2:30".into();
        db.reconcile(&actual).unwrap();
        assert!(!Provenance::sources(&path).unwrap().contains_key("editor"));
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
