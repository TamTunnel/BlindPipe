pub mod metadata;
pub mod statistical;
pub mod unicode;

use crate::utils::json_walker::StringProcessor;
use crate::vault::Vault;
use metadata::MetadataStripper;
use statistical::disrupt_watermark;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use unicode::clean_unicode;

pub struct InboundPipeline {
    pub vault: Arc<Vault>,
}

impl InboundPipeline {
    pub fn new(vault: Arc<Vault>) -> Self {
        Self { vault }
    }
}

pub struct SessionInbound<'a> {
    pub pipeline: &'a InboundPipeline,
    pub session_id: &'a str,
}

impl<'a> StringProcessor for SessionInbound<'a> {
    fn process<'b>(&'b self, s: &'b str) -> Pin<Box<dyn Future<Output = String> + Send + 'b>> {
        Box::pin(async move {
            let mut text = self.pipeline.vault.desanitize(self.session_id, s).await;
            text = clean_unicode(&text);
            text = disrupt_watermark(&text);
            text = MetadataStripper::strip_metadata(&text);
            text
        })
    }
}
