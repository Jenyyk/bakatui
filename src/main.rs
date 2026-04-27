mod persistent;
mod timetable;
mod tui;

use crate::persistent::Persistent;
use crate::tui::BackendCommand;
use serde::{Deserialize, Serialize};
use timetable::Timetable;

use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Default, Deserialize, Serialize)]
struct Config {
    url: String,
    user: String,
    password: String,
    refresh_token: Option<String>,
}

#[derive(Default, Deserialize, Serialize)]
struct Cache {
    timetable_data: String,
}

struct Extra {
    frontend_sender: Sender<FrontendCommand>,
}

enum FrontendCommand {
    TimetableData(Timetable),
    Log(String),
    Quit,
}

#[tokio::main]
async fn main() {
    let print_json_and_exit = std::env::var("JSON_DEBUG")
        .map(|v| v.parse::<i64>().unwrap())
        .ok();

    let mut persistent = Persistent::load_or_create();
    persistent.config.refresh_token = None;

    let (ttx, trx): (Sender<FrontendCommand>, Receiver<FrontendCommand>) = mpsc::channel();
    let (btx, brx): (Sender<BackendCommand>, Receiver<BackendCommand>) = mpsc::channel();

    ensure_config_filled_from_user(&mut persistent.config);
    let _ = persistent.save();

    if print_json_and_exit.is_none() {
        tokio::task::spawn(tui::tui_loop(trx, btx));
    }

    if let Ok(timetable_json) = serde_json::from_str(&persistent.cache.timetable_data) {
        let timetable = Timetable::from_json(timetable_json);
        let _ = ttx.send(FrontendCommand::TimetableData(timetable));
    }

    let access_token = match get_access_token(&mut persistent.config).await {
        Ok(token) => token,
        Err(err) => {
            match err {
                LoginError::Other(msg) => {
                    let _ = ttx.send(FrontendCommand::Log(format!(
                        "Login failed with unknown error, backend thread exiting. Error: {}",
                        msg
                    )));
                    panic!();
                }
                LoginError::BadLogin => {
                    let _ = ttx.send(FrontendCommand::Log("Login failed due to wrong credentials, please relaunch app and log in again".into()));
                    persistent.config.password = "".into();
                    persistent.config.user = "".into();
                    persistent.config.url = "".into();
                    persistent.config.refresh_token = None;
                    let _ = persistent.save();
                    panic!();
                }
            }
        }
    };
    let timetable_json = get_timetable(
        &persistent.config,
        access_token,
        print_json_and_exit.unwrap_or(0),
    )
    .await
    .unwrap();

    if print_json_and_exit.is_some() {
        println!("{:?}", timetable_json);
        return;
    }

    persistent.cache.timetable_data = timetable_json.to_string();

    let timetable = Timetable::from_json(timetable_json);

    let _ = ttx.send(FrontendCommand::TimetableData(timetable));

    let mut extra = Extra {
        frontend_sender: ttx,
    };

    loop {
        let command = brx.recv().unwrap();
        match command {
            BackendCommand::Quit => break,
            _ => handle_backend_command(command, &mut persistent, &mut extra).await,
        }
    }

    persistent.save().unwrap();
}

/// attempts to get an access token
///
/// if the attempt was made with an invalid refresh token, it will refresh the refresh token
async fn get_access_token(config: &mut Config) -> Result<String, LoginError> {
    let body = match &config.refresh_token {
        Some(token) => format!(
            "client_id=ANDR&grant_type=refresh_token&refresh_token={}",
            token
        ),
        None => format!(
            "client_id=ANDR&grant_type=password&username={}&password={}",
            config.user, config.password
        ),
    };
    let url = format!("{}/api/login", config.url);

    let client = reqwest::Client::new();

    let response = client
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?;

    if response.status().is_success() {
        let resp_json: ApiLoginResponse = response.json().await?;
        config.refresh_token = Some(resp_json.refresh_token);
        return Ok(resp_json.access_token);
    }

    if response.status() == reqwest::StatusCode::BAD_REQUEST {
        let err_json: ApiErrorResponse = response.json().await?;
        if &err_json.error_description == "The specified token is invalid."
            && config.refresh_token.is_some()
        {
            // the refresh token has been invalidates and we need to restart it
            config.refresh_token = None;
            return Box::pin(get_access_token(config)).await;
        }
        return Err(LoginError::BadLogin);
    }

    Err(LoginError::Other(format!(
        "Unexpected error while logging in: {:?}",
        response
    )))
}
#[derive(Debug)]
enum LoginError {
    Other(String),
    BadLogin,
}
impl From<reqwest::Error> for LoginError {
    fn from(err: reqwest::Error) -> Self {
        LoginError::Other(err.to_string())
    }
}

async fn get_timetable(
    config: &Config,
    access_token: String,
    week_modifier: i64,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let second_modifier =
        std::time::Duration::from_secs(i64::abs(week_modifier * 7 * 24 * 60 * 60) as u64);

    let now = chrono::Utc::now();
    let date = if week_modifier >= 0 {
        now + second_modifier
    } else {
        now - second_modifier
    };

    let url = format!(
        "{}/api/3/timetable/actual?date={}",
        config.url,
        date.format("%Y-%m-%d")
    );

    let client = reqwest::Client::new();

    let response = client
        .get(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to get timetable: {}\n{}",
            response.status(),
            response.text().await.unwrap()
        )
        .into());
    }

    let val: serde_json::Value = response.json().await?;
    Ok(val)
}

#[derive(Deserialize)]
struct ApiLoginResponse {
    access_token: String,
    refresh_token: String,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: String,
    error_description: String,
}

fn ensure_config_filled_from_user(config: &mut Config) {
    let stdin = std::io::stdin();
    if config.url.is_empty() {
        println!("Zadej URL školy:");
        stdin.read_line(&mut config.url).unwrap();
        config.url = config.url.trim().to_string();
    }
    if config.user.is_empty() {
        println!("Zadej uživatelské jméno:");
        stdin.read_line(&mut config.user).unwrap();
        config.user = config.user.trim().to_string();
    }
    if config.password.is_empty() {
        println!("Zadej heslo:");
        stdin.read_line(&mut config.password).unwrap();
        config.password = config.password.trim().to_string();
    }
}

async fn handle_backend_command(
    command: BackendCommand,
    persistent: &mut Persistent,
    extra: &mut Extra,
) {
    match command {
        BackendCommand::RefreshTimetable(week_modifier) => {
            let config = &mut persistent.config;
            let access_token = get_access_token(config).await.unwrap();
            let timetable_json = get_timetable(config, access_token, week_modifier)
                .await
                .unwrap();
            if week_modifier == 0 {
                persistent.cache.timetable_data = timetable_json.to_string();
            }
            let timetable: Timetable = Timetable::from_json(timetable_json);
            let _ = extra
                .frontend_sender
                .send(FrontendCommand::TimetableData(timetable));
        }
        BackendCommand::Quit => unreachable!(),
    }
}
