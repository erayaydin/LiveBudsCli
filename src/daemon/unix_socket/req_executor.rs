use super::connection_handler::get_err;
use super::{Request, Response};

use super::super::buds_config::Config;
use super::super::buds_info::{BudsInfo, BudsInfoInner};
use super::super::utils;

use async_std::sync::{Arc, Mutex};
use galaxy_buds_live_rs::message::{
    bud_property::{BudProperty, EqualizerType, Side, TouchpadOption},
    lock_touchpad, set_noise_reduction, set_touchpad_option,
    simple::new_equalizer,
};

// Set the value of a config option for a device
pub async fn set_config_value(
    payload: &Request,
    address: String,
    config: Arc<Mutex<Config>>,
) -> String {
    let mut config = config.lock().await;

    // Check if device already has a config entry
    // (should be available but you never know)
    if !config.has_device_config(&address) {
        return get_err("Device has no config!");
    }

    // Check required fields set
    if payload.opt_param1.is_none() || payload.opt_param2.is_none() {
        return get_err("Missing parameter");
    }

    let key = payload.opt_param1.clone().unwrap();
    let value = utils::str_to_bool(payload.opt_param2.clone().unwrap());

    // Get the right config entry mutable
    let cfg = config.get_device_config_mut(&address);
    if cfg.is_none() {
        return get_err("error getting config!");
    }
    let mut cfg = cfg.unwrap();

    // Set the right value of the config
    match key.as_str() {
        "auto_pause" => cfg.auto_pause_music = Some(value),
        "auto_play" => cfg.auto_resume_music = Some(value),
        "low_battery_notification" => cfg.low_battery_notification = Some(value),
        _ => {
            return get_err("Invalid key");
        }
    }

    // Try to save the config
    if let Err(err) = config.save().await {
        return get_err(format!("Err saving config: {}", err).as_str());
    }

    let a: Response<BudsInfoInner> = Response::new_success(address.clone(), None);
    serde_json::to_string(&a).unwrap()
}

pub async fn toggle_buds_value(payload: &Request, device_data: &mut BudsInfo) -> String {
    // Check required fields set
    if payload.opt_param1.is_none() {
        return get_err("Missing parameter");
    }

    let key = payload.opt_param1.clone().unwrap();
    let value = {
        match key.as_str() {
            "noise_reduction" => (!device_data.inner.noise_reduction).to_string(),
            "lock_touchpad" => (!device_data.inner.touchpads_blocked).to_string(),
            _ => {
                return get_err("Invalid key");
            }
        }
    };

    // Run desired command
    let res = set_buds_option(
        key.as_str(),
        value.as_str(),
        device_data,
        &payload.opt_param3,
    )
    .await;

    // Return success or error based on the success of the set command
    if res.is_ok() {
        let a: Response<BudsInfoInner> =
            Response::new_success(device_data.inner.address.clone(), None);
        serde_json::to_string(&a).unwrap()
    } else {
        get_err(res.err().unwrap().as_str())
    }
}

pub async fn set_buds_value(payload: &Request, device_data: &mut BudsInfo) -> String {
    // Check required fields set
    if payload.opt_param1.is_none() || payload.opt_param2.is_none() {
        return get_err("Missing parameter");
    }

    let key = payload.opt_param1.clone().unwrap();
    let value = payload.opt_param2.clone().unwrap();

    // Run desired command
    let res = set_buds_option(
        key.as_str(),
        value.as_str(),
        device_data,
        &payload.opt_param3,
    )
    .await;

    // Return success or error based on the success of the set command
    if res.is_ok() {
        let a: Response<BudsInfoInner> =
            Response::new_success(device_data.inner.address.clone(), None);
        serde_json::to_string(&a).unwrap()
    } else {
        get_err(res.err().unwrap().as_str())
    }
}

// Set the actual value
pub async fn set_buds_option(
    key: &str,
    value: &str,
    device_data: &mut BudsInfo,
    opt_param3: &Option<String>,
) -> Result<(), String> {
    match key {
        // Set noise reduction
        "noise_reduction" => {
            let value = utils::str_to_bool(&value);
            let msg = set_noise_reduction::new(value);
            let res = device_data.send(msg).await;
            if res.is_ok() {
                device_data.inner.noise_reduction = value;
            }
            res
        }

        // Set Touchpad lock
        "lock_touchpad" => {
            let value = utils::str_to_bool(&value);
            let msg = lock_touchpad::new(value);
            let res = device_data.send(msg).await;
            if res.is_ok() {
                device_data.inner.touchpads_blocked = value;
            }
            res
        }

        // Set EqualizerType command
        "equalizer" => match value.parse::<u8>() {
            Ok(val) => {
                let eq_type = EqualizerType::decode(val);
                let res = device_data.send(new_equalizer(eq_type)).await;
                if res.is_ok() {
                    device_data.inner.equalizer_type = eq_type;
                }
                res
            }
            Err(_) => Err("could not parse value".to_string()),
        },

        // Touchpad option
        "touchpad_action" => match value.parse::<u8>() {
            Ok(val) => {
                let option = TouchpadOption::decode(val);
                let mut left = device_data.inner.touchpad_option_left;
                let mut right = device_data.inner.touchpad_option_right;

                if opt_param3.is_some() {
                    let side = utils::str_to_side(opt_param3.as_ref().unwrap());
                    if side.is_none() {
                        return Err("Invalid side".to_string());
                    }
                    match side.unwrap() {
                        Side::Left => left = option,
                        Side::Right => right = option,
                    }
                } else {
                    left = option;
                    right = option;
                }

                let msg = set_touchpad_option::new(left, right);
                let res = device_data.send(msg).await;
                if res.is_ok() {
                    device_data.inner.touchpad_option_left = left;
                    device_data.inner.touchpad_option_right = right;
                }
                res
            }
            Err(_) => Err("could not parse value".to_string()),
        },
        _ => Err("Invalid key to set to".to_string()),
    }
}
