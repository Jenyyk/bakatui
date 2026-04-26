#![allow(unused)]

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, de::Visitor};

#[derive(Debug, Default)]
pub struct Timetable {
    pub hour_captions: Vec<String>,
    pub days: Vec<Day>,
}

#[derive(Debug)]
pub struct Day {
    pub day_type: DayType,
    pub hours: HashMap<String, Hour>,
}

#[derive(Clone, Debug)]
pub struct Hour {
    pub subject_short: String,
    pub subject_long: String,

    pub teacher_short: String,
    pub teacher_long: String,

    pub room_short: String,
    pub room_long: String,

    pub change: Option<Change>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Change {
    #[serde(rename = "ChangeType")]
    pub change_type: ChangeType,

    #[serde(rename = "Description")]
    pub description: String,

    #[serde(rename = "TypeAbbrev")]
    pub type_abbrev: Option<String>,
    #[serde(rename = "TypeName")]
    pub type_name: Option<String>,
}

#[derive(Debug)]
pub enum DayType {
    WorkDay,
    Weekend,
    Celebration,
    Holiday,
    DirectorDay,
    Undefined,
}

#[derive(Clone, Debug)]
pub enum ChangeType {
    Canceled,
    Added,
    Removed,
    RoomChanged,
    Substitution,
}

impl Timetable {
    pub fn from_json(json: serde_json::Value) -> Self {
        let id_map = construct_id_map(&json);
        let caption_map = construct_caption_map(&json);

        let hour_captions: Vec<String> = json["Hours"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["Caption"].as_str().unwrap().to_string())
            .collect();

        let mut days: Vec<Day> = Vec::new();
        for d in json["Days"].as_array().unwrap() {
            let day_type: DayType = d["DayType"].as_str().unwrap().into();

            let mut hours: HashMap<String, Hour> = HashMap::new();
            for h in d["Atoms"].as_array().unwrap() {
                let change: Option<Change> = serde_json::from_value(h["Change"].clone()).ok();
                let subject = id_map
                    .get(h["SubjectId"].as_str().unwrap_or("?"))
                    .cloned()
                    .unwrap_or(IdData {
                        short_name: change
                            .as_ref()
                            .and_then(|c| c.type_abbrev.to_owned())
                            .unwrap_or("?".into()),
                        long_name: change
                            .as_ref()
                            .and_then(|c| c.type_name.to_owned())
                            .unwrap_or("?".into()),
                    });
                let teacher = id_map
                    .get(h["TeacherId"].as_str().unwrap_or("?"))
                    .cloned()
                    .unwrap_or(Default::default());
                let room = id_map
                    .get(h["RoomId"].as_str().unwrap_or("?"))
                    .cloned()
                    .unwrap_or(Default::default());
                let caption = caption_map
                    .get(&h["HourId"].as_u64().unwrap())
                    .unwrap()
                    .clone();
                hours.insert(
                    caption.clone(),
                    Hour {
                        subject_short: subject.short_name.clone(),
                        subject_long: subject.long_name.clone(),
                        teacher_short: teacher.short_name.clone(),
                        teacher_long: teacher.long_name.clone(),
                        room_short: room.short_name.clone(),
                        room_long: room.long_name.clone(),
                        change,
                    },
                );
            }

            let day = Day { day_type, hours };
            days.push(day);
        }

        Self {
            hour_captions,
            days,
        }
    }
}

#[derive(Clone)]
struct IdData {
    short_name: String,
    long_name: String,
}

impl Default for IdData {
    fn default() -> Self {
        Self {
            short_name: "?".into(),
            long_name: "?".into(),
        }
    }
}

fn construct_id_map(json: &serde_json::Value) -> HashMap<String, IdData> {
    let mut id_map: HashMap<String, IdData> = HashMap::new();

    for subject in json["Subjects"].as_array().unwrap() {
        let id = subject["Id"].as_str().unwrap();
        id_map.insert(
            id.to_string(),
            IdData {
                short_name: subject["Abbrev"].as_str().unwrap().to_string(),
                long_name: subject["Name"].as_str().unwrap().to_string(),
            },
        );
    }

    for teacher in json["Teachers"].as_array().unwrap() {
        let id = teacher["Id"].as_str().unwrap();
        id_map.insert(
            id.to_string(),
            IdData {
                short_name: teacher["Abbrev"].as_str().unwrap().to_string(),
                long_name: teacher["Name"].as_str().unwrap().to_string(),
            },
        );
    }

    for room in json["Rooms"].as_array().unwrap() {
        let id = room["Id"].as_str().unwrap();
        id_map.insert(
            id.to_string(),
            IdData {
                short_name: room["Abbrev"].as_str().unwrap().to_string(),
                long_name: room["Name"].as_str().unwrap().to_string(),
            },
        );
    }

    id_map
}

fn construct_caption_map(json: &serde_json::Value) -> HashMap<u64, String> {
    let mut caption_map: HashMap<u64, String> = HashMap::new();

    for hour in json["Hours"].as_array().unwrap() {
        let id = hour["Id"].as_u64().unwrap();
        let caption = hour["Caption"].as_str().unwrap();
        caption_map.insert(id, caption.to_string());
    }

    caption_map
}

impl From<String> for DayType {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl From<&str> for DayType {
    fn from(s: &str) -> Self {
        match s {
            "WorkDay" => DayType::WorkDay,
            "Weekend" => DayType::Weekend,
            "Celebration" => DayType::Celebration,
            "Holiday" => DayType::Holiday,
            "DirectorDay" => DayType::DirectorDay,
            _ => DayType::Undefined,
        }
    }
}

impl From<String> for ChangeType {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl From<&str> for ChangeType {
    fn from(s: &str) -> Self {
        match s {
            "Canceled" => ChangeType::Canceled,
            "Added" => ChangeType::Added,
            "Removed" => ChangeType::Removed,
            "RoomChanged" => ChangeType::RoomChanged,
            "Substitution" => ChangeType::Substitution,
            _ => unreachable!(),
        }
    }
}

impl From<&ChangeType> for crossterm::style::Color {
    fn from(val: &ChangeType) -> Self {
        match val {
            ChangeType::Canceled => crossterm::style::Color::Green,
            ChangeType::Added => crossterm::style::Color::Red,
            ChangeType::Removed => crossterm::style::Color::Green,
            ChangeType::RoomChanged => crossterm::style::Color::Red,
            ChangeType::Substitution => crossterm::style::Color::Red,
        }
    }
}

impl From<&DayType> for crossterm::style::Color {
    fn from(val: &DayType) -> Self {
        match val {
            DayType::WorkDay => crossterm::style::Color::Reset,
            DayType::Weekend => crossterm::style::Color::Green,
            DayType::Celebration => crossterm::style::Color::Cyan,
            DayType::Holiday => crossterm::style::Color::Cyan,
            DayType::DirectorDay => crossterm::style::Color::Green,
            DayType::Undefined => crossterm::style::Color::DarkRed,
        }
    }
}

impl<'de> Deserialize<'de> for ChangeType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(ChangeType::from(s))
    }
}

use std::fmt::{Display, Formatter};
impl Display for Hour {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut base = format!(
            "Předmět:  {}\nUčitel:   {}\nMístnost: {}",
            self.subject_long, self.teacher_long, self.room_short
        );

        if let Some(change) = &self.change {
            let addition = format!(
                "\n\nZměna:    {:?}\nPopis:    {}",
                change.change_type, change.description
            );
            base.push_str(&addition);
        }

        write!(f, "{}", base)
    }
}
