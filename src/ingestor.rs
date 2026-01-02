use crate::db::Database;
use crate::events::Event;
use crate::nntp::NntpClient;
use crate::settings::{IngestionMode, Settings};
use anyhow::{anyhow, Result};
use tokio::sync::mpsc::Sender;
use tokio::time::{Duration, sleep};
use tracing::{error, info};

pub struct Ingestor {
    settings: Settings,
    db: Database,
    sender: Sender<Event>,
}

impl Ingestor {
    pub fn new(settings: Settings, db: Database, sender: Sender<Event>) -> Self {
        Self {
            settings,
            db,
            sender,
        }
    }

    pub async fn run(&self) -> Result<()> {
        match self.settings.ingestion.mode {
            IngestionMode::Nntp => self.run_nntp().await,
            IngestionMode::LocalArchive => self.run_local_archive().await,
        }
    }

    async fn run_nntp(&self) -> Result<()> {
        info!(
            "Starting NNTP Ingestor for groups: {:?}",
            self.settings.nntp.groups
        );

        loop {
            if let Err(e) = self.process_nntp_cycle().await {
                error!("NNTP Ingestion cycle failed: {}", e);
            }
            sleep(Duration::from_secs(60)).await;
        }
    }

    async fn process_nntp_cycle(&self) -> Result<()> {
        let mut client = NntpClient::connect(&self.settings.nntp.server, self.settings.nntp.port).await?;

        for group_name in &self.settings.nntp.groups {
            self.db.ensure_mailing_list(group_name, group_name).await?;

            let info = client.group(group_name).await?;
            let last_known = self.db.get_last_article_num(group_name).await?;

            info!(
                "Group {}: estimated count={}, low={}, high={}, last_known={}",
                group_name, info.number, info.low, info.high, last_known
            );

            let mut current = last_known;
            if current == 0 && info.high > 0 {
                current = info.high.saturating_sub(5);
                self.db.update_last_article_num(group_name, current).await?;
                info!("Initialized high-water mark to {}", current);
            }

            if current < info.high {
                let next_id = current + 1;
                info!("Fetching article {}", next_id);
                match client.article(&next_id.to_string()).await {
                    Ok(lines) => {
                        self.sender
                            .send(Event::ArticleFetched {
                                group: group_name.clone(),
                                article_id: next_id.to_string(),
                                content: lines,
                            })
                            .await?;
                        self.db.update_last_article_num(group_name, next_id).await?;
                        info!("Updated high-water mark to {}", next_id);
                    }
                    Err(e) => {
                        error!("Failed to fetch article {}: {}", next_id, e);
                    }
                }
            }
        }

        client.quit().await?;
        Ok(())
    }

    async fn run_local_archive(&self) -> Result<()> {
        let archive_settings = self.settings.ingestion.archive.as_ref()
            .ok_or_else(|| anyhow!("LocalArchive mode selected but no archive path provided"))?;
        
        info!("Starting Local Archive Ingestor from {:?}", archive_settings.path);

        // Placeholder for local archive ingestion logic
        // 1. Walk the directory or open git repo
        // 2. Parse emails
        // 3. Send events
        
        Ok(())
    }
}