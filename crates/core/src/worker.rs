//! Background processing loop. One job at a time; resumes on startup.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

use crate::secrets::{SecretStore, build_processors};
use crate::{Db, jobs, pipeline, settings};

/// Called after a job changes state, with the job's target id.
pub type OnUpdate = Arc<dyn Fn(&str) + Send + Sync>;

pub struct Worker {
    pub db: Arc<Db>,
    pub secrets: Arc<dyn SecretStore>,
    pub notify: Arc<Notify>,
    pub on_update: OnUpdate,
}

impl Worker {
    pub async fn run(self) {
        if let Err(e) = jobs::reset_running(&self.db.conn()) {
            eprintln!("worker: could not reset interrupted jobs: {e:#}");
        }
        loop {
            let processors = {
                let s = settings::get(&self.db.conn());
                match s {
                    Ok(s) => build_processors(&s, &*self.secrets),
                    Err(e) => Err(e.to_string()),
                }
            };
            let Ok(p) = processors else {
                // Blocked (no key/model, or paused): jobs stay queued.
                self.wait(Duration::from_secs(30)).await;
                continue;
            };
            let job = jobs::claim_next(&self.db.conn());
            match job {
                Ok(Some(job)) => {
                    (self.on_update)(&job.target_id);
                    let res = pipeline::run_job(&self.db, &p, &job).await;
                    {
                        let conn = self.db.conn();
                        let r = match res {
                            Ok(()) => jobs::complete(&conn, &job.id),
                            Err(e) => jobs::fail(&conn, &job, &format!("{e:#}")),
                        };
                        if let Err(e) = r {
                            eprintln!("worker: could not record job result: {e:#}");
                        }
                    }
                    (self.on_update)(&job.target_id);
                }
                Ok(None) => self.wait(Duration::from_secs(15)).await,
                Err(e) => {
                    eprintln!("worker: claim failed: {e:#}");
                    self.wait(Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn wait(&self, d: Duration) {
        tokio::select! {
            _ = self.notify.notified() => {}
            _ = tokio::time::sleep(d) => {}
        }
    }
}
