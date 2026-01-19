use std::{
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::SystemTime,
};

pub use anyhow;
pub use discord_sdk as ds;
use discord_sdk::activity::Button;
pub use tokio;

pub struct Discord {
    rt: tokio::runtime::Runtime,
    details: Arc<Mutex<String>>,
    state: Arc<Mutex<String>>,
    client: Option<Arc<Client>>,
    running: Arc<AtomicBool>,
    initialized: Arc<AtomicBool>,
}

impl Discord {
    pub fn new() -> Result<Self, anyhow::Error> {
        Ok(Self {
            rt: tokio::runtime::Runtime::new()?,
            details: Arc::new(Mutex::new(String::new())),
            state: Arc::new(Mutex::new(String::new())),
            client: None,
            running: Arc::new(AtomicBool::new(false)),
            initialized: Arc::new(AtomicBool::new(false)),
        })
    }
    pub fn init(&mut self) {
        if self.client.is_none() {
            let (client, success) = self.rt.block_on(make_client(ds::Subscriptions::ACTIVITY));
            self.client = Some(Arc::new(client));
            self.initialized
                .store(success, std::sync::atomic::Ordering::Release);
        }

        let time_stamp = SystemTime::now();
        let details = self.details.clone();
        let state = self.state.clone();
        let running = self.running.clone();
        let initialized = self.initialized.clone();
        let client = self.client.as_ref().unwrap().clone();
        self.running
            .store(true, std::sync::atomic::Ordering::Release); //idk what correct ordering is here
        self.rt.spawn(async move {
            loop {
                if !running.load(std::sync::atomic::Ordering::Acquire) {
                    break;
                }

                // Only update activity if Discord was successfully initialized
                if initialized.load(std::sync::atomic::Ordering::Acquire) {
                    let rp = ds::activity::ActivityBuilder::default()
                        .details(details.lock().unwrap().as_str())
                        .state(state.lock().unwrap().as_str())
                        .button(Button {
                            label: "noctismenu.dev".to_string(),
                            url: crate::LOADER_WEBSITE.to_string(),
                        })
                        .assets(ds::activity::Assets::default().large(
                            "logo".to_owned(),
                            Some(crate::LOADER_WEBSITE.strip_prefix("https://").unwrap()),
                        ))
                        .start_timestamp(time_stamp);
                    log::info!(
                        "updated activity: {:?}",
                        client.discord.update_activity(rp).await
                    );
                }

                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
    }

    pub fn set_details(&mut self, details: impl Into<String>) {
        *self.details.lock().unwrap() = details.into();
    }

    pub fn set_state(&mut self, state: impl Into<String>) {
        *self.state.lock().unwrap() = state.into();
    }

    pub fn disable(&mut self) {
        log::info!("Disabling!!");
        self.running
            .store(false, std::sync::atomic::Ordering::Release);

        // Only clear activity if Discord was successfully initialized
        if self.initialized.load(std::sync::atomic::Ordering::Acquire) {
            let client = self.client.clone().unwrap();
            self.rt.spawn(async move {
                log::info!(
                    "clearing activity: {:?}",
                    client.discord.clear_activity().await
                );
            });
        }
    }

    pub fn running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}

pub struct Client {
    pub discord: ds::Discord,
    pub user: ds::user::User,
    pub wheel: ds::wheel::Wheel,
}

fn dummy_user() -> ds::user::User {
    ds::user::User {
        id: ds::Snowflake(0),
        username: "Unknown".to_string(),
        discriminator: None,
        avatar: None,
        is_bot: false,
    }
}

pub async fn make_client(subs: ds::Subscriptions) -> (Client, bool) {
    let (wheel, handler) = ds::wheel::Wheel::new(Box::new(|err| {
        log::error!("encountered an error: {:?}", err);
    }));

    let mut user = wheel.user();

    let discord = ds::Discord::new(
        ds::DiscordApp::PlainId(crate::DISCORD_APP_ID),
        subs,
        Box::new(handler),
    )
    .expect("unable to create discord client");

    log::info!("waiting for handshake...");

    // Wait for handshake with a 15-second timeout
    let handshake_result =
        tokio::time::timeout(std::time::Duration::from_secs(15), user.0.changed()).await;

    let (user, success) = match handshake_result {
        Ok(Ok(())) => {
            match &*user.0.borrow() {
                ds::wheel::UserState::Connected(user) => {
                    log::info!("connected to Discord, local user is {:#?}", user);
                    (user.clone(), true)
                }
                ds::wheel::UserState::Disconnected(err) => {
                    log::error!("failed to connect to Discord: {}", err);
                    // Return a dummy user and mark as failed
                    (dummy_user(), false)
                }
            }
        }
        Ok(Err(e)) => {
            log::error!("error during handshake: {:?}", e);
            (dummy_user(), false)
        }
        Err(_) => {
            log::warn!("Discord handshake timed out after 15 seconds, continuing anyway");
            (dummy_user(), false)
        }
    };

    (
        Client {
            discord,
            user,
            wheel,
        },
        success,
    )
}
