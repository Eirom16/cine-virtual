//! JNI is only an Android SDK driver/owner shim over the same Application boundary.
use super::*;
use jni::{
    JNIEnv,
    objects::{JObject, JString},
    sys::{jlong, jstring},
};
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_cinevirtual_cine_1mobile_1spike_MainActivity_nativeClock(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
) -> jlong {
    registry()
        .lock()
        .unwrap()
        .get(&(handle as u64))
        .map_or(-1, |i| i.started.elapsed().as_millis() as jlong)
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_cinevirtual_cine_1mobile_1spike_MainActivity_nativeDestroy(
    _env: JNIEnv,
    _this: JObject,
    handle: jlong,
) {
    cine_bridge_destroy(handle as u64);
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_cinevirtual_cine_1mobile_1spike_MainActivity_nativeDrive(
    mut env: JNIEnv,
    _this: JObject,
    handle: jlong,
    sample: JString,
) -> jstring {
    let entry = std::time::Instant::now();
    let response=(||{
        let data:String=env.get_string(&sample).ok()?.into();
        if data.len()>4096{return None;}
        let value:serde_json::Value=serde_json::from_str(&data).ok()?;
        let mut instances=registry().lock().unwrap();let i=instances.get_mut(&(handle as u64))?;
        let now=i.started.elapsed().as_millis() as u64;
        let s:Sample=serde_json::from_value(value["sample"].clone()).ok()?;
        if s.position_ms>s.duration_ms || s.duration_ms>604_800_000 || s.age_ms>100 {return None;}
        let rate=value["rate"].as_f64()?;
        if !rate.is_finite() || !(0.5..=2.0).contains(&rate){return None;}
        let n=i.app.network.as_ref()?;
        if let (Some(g),Some(op),Some(loss)) = (value["seek_loss"]["generation"].as_u64(),
            value["seek_loss"]["operation_id"].as_u64(),value["seek_loss"]["loss_ms"].as_u64()) {
            n.player.seek_loss(g,op,loss);
        }
        if !i.app.suspended {
            i.app.caps.playback_rate=value["supports_rate"].as_bool().unwrap_or(false);
            i.app.sample=s;
            n.player.sample(cine_client::player_backend::PlayerView { position_ms:s.position_ms,duration_ms:s.duration_ms,playing:s.playing,ready:s.loaded,buffering:s.buffering,seeking:s.seeking,
                sampled_at_ms:now.saturating_sub(s.age_ms),rate,failed:value["failed"].as_bool().unwrap_or(false)},i.app.caps.playback_rate);
        }
        let (diagnostics,dropped)=n.player.drain_diagnostics();
        Some(serde_json::json!({"generation":i.app.generation,"now_ms":now,"effects":n.player.drain(),"diagnostics":diagnostics,
            "diagnostics_dropped":dropped,"jni_to_lock_ms":entry.elapsed().as_secs_f64()*1000.0,
            "source_mapped_at_ms":value["source_mapped_at_ms"],"sample_assigned_at_ms":now.saturating_sub(s.age_ms)}))
    })().unwrap_or_else(||serde_json::json!({"effects":[]}));
    env.new_string(response.to_string())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
