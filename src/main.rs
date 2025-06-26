use std::collections::{HashMap, HashSet};
use std::env;
use serenity::all::{GuildId, Member, PartialMember, ScheduledEvent, ScheduledEventId, ScheduledEventStatus, User, UserId};
use serenity::futures::channel;
use serenity::{async_trait, http};
use serenity::model::guild;
use serenity::model::{channel::Message, gateway::Ready};
use serenity::prelude::*;
use dotenv;
use lazy_static::lazy_static;

use std::sync::Mutex;


// A buffer for storing prior event states for comparison.
// Consider just saving the data you need using a custom struct.
lazy_static! {
    // static ref EVENT_BUFFER: Mutex<Vec<ScheduledEvent>> = Mutex::new(vec![]);
    static ref EVENT_BUFFER: Mutex<HashMap<ScheduledEventId, ScheduledEvent>> = Mutex::new(HashMap::new());
}

struct Handler;

async fn handle_status_change(ctx: &Context, event: &ScheduledEvent) {
    if event.channel_id.is_none() { return }
    let channel = event.channel_id.unwrap().to_channel(&ctx.http()).await.unwrap().guild();
    let members_opt = match channel {
        Some(ref channel) => channel.members(&ctx).ok(),
        _ => None,
    };
    
    let guild_id = GuildId::new(env::var("GUILD_ID").expect("GUILD_ID missing!").parse().unwrap());
    let interested_users = ctx.http().get_scheduled_event_users(guild_id, event.id, None, None, Some(true)).await.unwrap();

    if members_opt.is_none() {return}
    let members =  members_opt.unwrap();
    let mem_ids: HashSet<u64> = members.iter().map(|mem| mem.user.id.get()).collect();
    let user_ids: HashSet<u64> = interested_users.iter().map(|user| user.user.id.get() ).collect();
    
    spawn_threads_late_users(&ctx, user_ids.difference(&mem_ids).collect()).await;
            
}

async fn spawn_threads_late_users(ctx: &Context, user_ids: Vec<&u64>) {

}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, _ctx: Context, _msg: Message) {
        // we in general don't need to do anything with the messages right now
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
        
        // If using an event state buffer, populate it here by fetching current events from the API
        // This allows comparing future updates with their previous state, even across restarts
        let guild_id = GuildId::new(env::var("GUILD_ID").expect("GUILD_ID missing!").parse().unwrap());
        let events = ctx.http.get_scheduled_events(guild_id, true).await.unwrap();

        let mut event_buffer = EVENT_BUFFER.lock().unwrap();
        for event in events.clone().iter() {
            event_buffer
                .insert(event.id, event.to_owned());
            println!("{}", event.id.to_string());
        }   
        println!("test event: {:?}", event_buffer.get(&events[0].id).unwrap().name);
    }

    async fn guild_scheduled_event_update(&self, ctx: Context, event: ScheduledEvent) {
        // Discord doesn't include prior event state in update payloads
        // We must cache the previous state ourselves, e.g., in a HashMap
        // Alternatively, fetch full event list on each update and diff, but this is costly and still can't recover past state
        // Memory/disk-backed buffer is most reliable

         
        // Accessing buffer
        let mut prior_event: Option<ScheduledEvent> = None;
        if event.status == ScheduledEventStatus::Active {
            // Accessing EVENT_BUFFER inside scope to avoid locking it for the entire duration of the function
            let event_buffer = EVENT_BUFFER.lock().unwrap();
            let event = event_buffer.get(&event.id);
            if event.is_some() {
                prior_event = Some(event.unwrap().to_owned());
            }
        }

        // If there's no prior event, it's guaranteed to just have been created or made public
        // in which case we can't compare it either way
        if prior_event.is_none() { return }
        else if prior_event.unwrap().status == ScheduledEventStatus::Scheduled && event.status == ScheduledEventStatus::Active {
            handle_status_change(&ctx, &event).await;
        }  

        EVENT_BUFFER.lock().unwrap().insert(event.id, event);

    }
}


#[tokio::main]
async fn main() {
    dotenv::from_filename(".env.local").ok();

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in ENV");
    let intents = GatewayIntents::GUILDS 
    | GatewayIntents::GUILD_SCHEDULED_EVENTS 
    | GatewayIntents::GUILD_MEMBERS 
    | GatewayIntents::GUILD_VOICE_STATES;

    let mut client = Client::builder(&token, intents).event_handler(Handler).await.expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
