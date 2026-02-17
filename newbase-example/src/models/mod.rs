#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case, unused)]

pub mod math;

#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u16)]
pub enum AbilitySlot {
    ESlot_Signature_1 = 0x0,
    ESlot_Signature_2 = 0x1,
    ESlot_Signature_3 = 0x2,
    ESlot_Signature_4 = 0x3,
    ESlot_ActiveItem_1 = 0x4,
    ESlot_ActiveItem_2 = 0x5,
    ESlot_ActiveItem_3 = 0x6,
    ESlot_ActiveItem_4 = 0x7,
    ESlot_Ability_Held = 0x8,
    ESlot_Ability_ZipLine = 0x9,
    ESlot_Ability_Mantle = 0xa,
    ESlot_Ability_ClimbRope = 0xb,
    ESlot_Ability_Jump = 0xc,
    ESlot_Ability_Slide = 0xd,
    ESlot_Ability_Teleport = 0xe,
    ESlot_Ability_ZipLineBoost = 0xf,
    ESlot_Ability_Innate_1 = 0x10,
    ESlot_Ability_Innate_2 = 0x11,
    ESlot_Ability_Innate_3 = 0x12,
    ESlot_Weapon_Secondary = 0x13,
    ESlot_Weapon_Primary = 0x14,
    ESlot_Weapon_Melee = 0x15,
    ESlot_None = 0x16, // EMaxAbilitySlots
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum EJumpType {
    EJumpType_Ground = 0,
    EJumpType_Air = 1,
    EJumpType_Wall = 2,
    EJumpType_DashJump = 3,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum Hero {
    #[default]
    None = 0,
    Infernus = 1,
    Seven = 2,
    Vindicta = 3,
    LadyGeist = 4,
    Abrams = 6,
    Wraith = 7,
    McGinnis = 8,
    Paradox = 10,
    Dynamo = 11,
    Kelvin = 12,
    Haze = 13,
    Holliday = 14,
    Bebop = 15,
    Calico = 16,
    GreyTalon = 17,
    MoAndKrill = 18,
    Shiv = 19,
    Ivy = 20,
    Viper = 21,
    Warden = 25,
    Yamato = 27,
    Lash = 31,
    Viscous = 35,
    Wrecker = 48,
    Pocket = 50,
    Mirage = 52,
    Fathom = 53,
    Dummy = 55,
    Magician = 60,
    Trapper = 61,
    Raven = 62,
    Mina = 63,
    Drifter = 64,
    Viktor = 66,
    Paige = 67,
    Doorman = 69,
    Billy = 72,
}

impl TryFrom<i32> for Hero {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Hero::Infernus),
            2 => Ok(Hero::Seven),
            3 => Ok(Hero::Vindicta),
            4 => Ok(Hero::LadyGeist),
            6 => Ok(Hero::Abrams),
            7 => Ok(Hero::Wraith),
            8 => Ok(Hero::McGinnis),
            10 => Ok(Hero::Paradox),
            11 => Ok(Hero::Dynamo),
            12 => Ok(Hero::Kelvin),
            13 => Ok(Hero::Haze),
            14 => Ok(Hero::Holliday),
            15 => Ok(Hero::Bebop),
            16 => Ok(Hero::Calico),
            17 => Ok(Hero::GreyTalon),
            18 => Ok(Hero::MoAndKrill),
            19 => Ok(Hero::Shiv),
            20 => Ok(Hero::Ivy),
            21 => Ok(Hero::Viper),
            25 => Ok(Hero::Warden),
            27 => Ok(Hero::Yamato),
            31 => Ok(Hero::Lash),
            35 => Ok(Hero::Viscous),
            48 => Ok(Hero::Wrecker),
            50 => Ok(Hero::Pocket),
            52 => Ok(Hero::Mirage),
            53 => Ok(Hero::Fathom),
            55 => Ok(Hero::Dummy),
            58 => Ok(Hero::Viper),
            60 => Ok(Hero::Magician),
            61 => Ok(Hero::Trapper),
            62 => Ok(Hero::Raven),
            63 => Ok(Hero::Mina),
            64 => Ok(Hero::Drifter),
            66 => Ok(Hero::Viktor),
            67 => Ok(Hero::Paige),
            69 => Ok(Hero::Doorman),
            72 => Ok(Hero::Billy),
            _ => {
                //log::error!("Unknown hero id: {}", value);
                Err(())
            }
        }
    }
}

impl Hero {
    pub fn get_head_bone(self) -> Option<i32> {
        match self {
            Hero::Infernus => Some(30),
            Hero::Seven => Some(14),
            Hero::Vindicta => Some(7),
            Hero::LadyGeist => Some(11),
            Hero::Abrams => Some(7),
            Hero::Wraith => Some(7),
            Hero::McGinnis => Some(38),
            Hero::Paradox => Some(8),
            Hero::Dynamo => Some(23),
            Hero::Kelvin => Some(12),
            Hero::Haze => Some(14),
            Hero::Holliday => Some(13),
            Hero::Bebop => Some(6),
            Hero::Calico => Some(13),
            Hero::GreyTalon => Some(17),
            Hero::MoAndKrill => Some(7),
            Hero::Shiv => Some(13),
            Hero::Ivy => Some(13),
            Hero::Viper => Some(13),
            Hero::Warden => Some(11),
            Hero::Yamato => Some(34),
            Hero::Lash => Some(12),
            Hero::Viscous => Some(7),
            Hero::Wrecker => Some(8),
            Hero::Pocket => Some(13),
            Hero::Mirage => Some(7),
            Hero::Fathom => Some(13),
            Hero::Dummy => Some(34),
            Hero::Magician => Some(7),
            Hero::Trapper => Some(13),
            Hero::Raven => Some(7),
            Hero::Mina => Some(16),
            Hero::Drifter => Some(18),
            Hero::Viktor => Some(58),
            Hero::Paige => Some(56),
            Hero::Doorman => Some(20),
            Hero::Billy => Some(7),
            _ => None,
        }
    }

    pub fn to_string(self) -> String {
        match self {
            Hero::None => "None",
            Hero::Infernus => "Infernus",
            Hero::Seven => "Seven",
            Hero::Vindicta => "Vindicta",
            Hero::LadyGeist => "LadyGeist",
            Hero::Abrams => "Abrams",
            Hero::Wraith => "Wraith",
            Hero::McGinnis => "McGinnis",
            Hero::Paradox => "Paradox",
            Hero::Dynamo => "Dynamo",
            Hero::Kelvin => "Kelvin",
            Hero::Haze => "Haze",
            Hero::Holliday => "Holliday",
            Hero::Bebop => "Bebop",
            Hero::Calico => "Calico",
            Hero::GreyTalon => "GreyTalon",
            Hero::MoAndKrill => "MoAndKrill",
            Hero::Shiv => "Shiv",
            Hero::Ivy => "Ivy",
            Hero::Viper => "Viper",
            Hero::Warden => "Warden",
            Hero::Yamato => "Yamato",
            Hero::Lash => "Lash",
            Hero::Viscous => "Viscous",
            Hero::Wrecker => "Wrecker",
            Hero::Pocket => "Pocket",
            Hero::Mirage => "Mirage",
            Hero::Fathom => "Fathom",
            Hero::Dummy => "Dummy",
            Hero::Magician => "Magician",
            Hero::Trapper => "Trapper",
            Hero::Raven => "Raven",
            Hero::Mina => "Mina",
            Hero::Drifter => "Drifter",
            Hero::Viktor => "Viktor",
            Hero::Paige => "Paige",
            Hero::Doorman => "Doorman",
            Hero::Billy => "Billy",
        }
        .to_string()
    }
}
