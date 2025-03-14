use dotenv::dotenv;
use serenity::all::{
    ChannelId, Context, CreateEmbed, CreateMessage, EmojiId, EventHandler, GatewayIntents, GuildId,
    Interaction, Message, MessageId, Reaction, ReactionType,
};
use serenity::async_trait;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;

struct Handler {
    reaction_map: Arc<Mutex<HashMap<MessageId, i32>>>,
    channel_id_map: Arc<Mutex<HashMap<GuildId, ChannelId>>>,
    conn: Arc<Mutex<sqlite::Connection>>,
}
#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, new_message: Message) {
        for channel_id in self.channel_id_map.lock().await.values() {
            println!("Channel ID: {}", channel_id);
        }
        if new_message.author.bot {
            return;
        }
        let guild_id = new_message.guild_id.unwrap();
        let guild = guild_id.to_partial_guild(&ctx).await.unwrap();
        let is_owner = guild.owner_id == new_message.author.id;

        println!("Owner: {}", is_owner);
        println!("Owner id: {}", guild.owner_id);
        println!("Author id: {}", new_message.author.id);
        println!("Content: {}", new_message.content);

        if is_owner && new_message.content.trim().starts_with(".setchanid") {
            println!("inside!");
            let guild_id = new_message.guild_id.unwrap();
            let channel_id = new_message.content.split_whitespace().nth(1).unwrap();
            let channel_id = ChannelId::new(channel_id.parse::<u64>().unwrap());
            let mut channel_id_map = self.channel_id_map.lock().await;
            channel_id_map.insert(guild_id, channel_id);

            let mut conn = self.conn.lock().await;
            conn.execute(
                format!(
                    "INSERT OR REPLACE INTO channel_id (guild_id, channel_id) VALUES ({}, {})",
                    guild_id.get() as i64,
                    channel_id.get() as i64
                )
                .as_str(),
            )
            .unwrap();

            println!("Channel ID set for guild: {}", guild_id);
        }
    }
    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        println!("Reaction added: {:?}", reaction.emoji);
        if let ReactionType::Custom { id: (ref id), .. } = reaction.emoji {
            if id.get() != 230782152164245505 {
                return;
            }
        }
        let mut reaction_map = self.reaction_map.lock().await;
        let counter = reaction_map.entry(reaction.message_id).or_insert(0);
        *counter += 1;
        if *counter == 5 {
            let mut channel_id_map = self.channel_id_map.lock().await;
            let channel_id = channel_id_map.get(&reaction.guild_id.unwrap());

            if channel_id.is_none() {
                return;
            }
            let channel_id = channel_id.unwrap();

            let message = reaction.message(&ctx).await.unwrap();

            let embed = CreateEmbed::new()
                .title("New Message")
                .description(format!("**{0}**\n {1}", message.author, message.content));
            let builder = CreateMessage::new().embed(embed);

            let _ = channel_id.send_message(&ctx.http, builder).await;
        }
    }
}
#[tokio::main]
async fn main() {
    let connection = sqlite::open("channel_id.db").unwrap();
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS channel_id (
            guild_id INTEGER PRIMARY KEY,
            channel_id INTEGER NOT NULL
        )",
        )
        .unwrap();
    dotenv().ok();
    let token = env::var("TOKEN").expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::MESSAGE_CONTENT;
    let handler = Handler {
        reaction_map: Arc::new(Mutex::new(HashMap::new())),
        channel_id_map: Arc::new(Mutex::new(HashMap::new())),
        conn: Arc::new(Mutex::new(connection)),
    };

    let query = "SELECT * FROM channel_id";

    for row in handler
        .conn
        .lock()
        .await
        .prepare(query)
        .unwrap()
        .into_iter()
        .map(|row| row.unwrap())
    {
        let guild_id = row.read::<i64, _>(0);
        let channel_id = row.read::<i64, _>(1);
        let guild_id = GuildId::new(guild_id as u64);
        let channel_id = ChannelId::new(channel_id as u64);
        let mut channel_id_map = handler.channel_id_map.lock().await;
        channel_id_map.insert(guild_id, channel_id);
    }

    let mut client = serenity::Client::builder(&token, intents)
        .event_handler(handler)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
