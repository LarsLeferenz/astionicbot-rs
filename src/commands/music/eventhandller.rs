use serenity::async_trait;
use songbird::{Event, EventContext, EventHandler};

pub struct CustomSongbirdEventHandler;

impl CustomSongbirdEventHandler {
    pub fn new() -> Self {
        Self
    }
}
#[async_trait]
impl EventHandler for CustomSongbirdEventHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        println!("{:?}", ctx);

        if let Some(evt) = ctx.to_core_event() {
            return Some(Event::Core(evt));
        }
        Option::None
    }
}
