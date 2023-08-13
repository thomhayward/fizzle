use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Status0 {
	#[serde(rename = "Status")]
	pub status: Status0Inner,
}

//  {"Status":{"Module":0,"DeviceName":"Tasmota","FriendlyName":["Tasmota"],"Topic":"power/rear-bedroom/socket-104","ButtonTopic":"0","Power":1,"PowerOnState":3,"LedState":1,"LedMask":"FFFF","SaveData":1,"SaveState":1,"SwitchTopic":"0","SwitchMode":[0,0,0,0,0,0,0,0],"ButtonRetain":0,"SwitchRetain":0,"SensorRetain":0,"PowerRetain":0}}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Status0Inner {
	#[serde(rename = "Module")]
	pub module: u32,
	#[serde(rename = "DeviceName")]
	pub device_name: String,
	#[serde(rename = "Topic")]
	pub topic: String,
}
