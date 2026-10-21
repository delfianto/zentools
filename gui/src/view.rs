//! Presentation values built from owned snapshots; unavailable readings remain null / N/A.
use serde_json::{Value, json};
use std::collections::VecDeque;
use zentools::{
    epp::EppProfile,
    monitor::{self, Snapshot},
};

#[derive(Default)]
pub struct State {
    pub latest: Option<Snapshot>,
    pub baseline: Option<Snapshot>,
    pub history: VecDeque<Value>,
    pub demo: bool,
    pub elevated: bool,
    pub authorizing: bool,
    pub paused: bool,
    pub message: String,
    pub message_error: bool,
}

impl State {
    pub fn message(&mut self, text: impl Into<String>, error: bool) {
        self.message = text.into();
        self.message_error = error;
    }
    pub fn accept(&mut self, snapshot: Snapshot) {
        if self.history.len() >= 120 {
            self.history.pop_front();
        }
        self.history.push_back(json!({"timestamp":snapshot.timestamp_ms,"power":snapshot.metrics.package_power_w,"temperature":snapshot.metrics.tctl_temp_c}));
        self.latest = Some(snapshot);
    }
    pub fn render(&self) -> String {
        let s = self.latest.as_ref();
        let m = s.map(|s| &s.metrics);
        let metric = |value: Option<f64>, unit: &str, detail: &str| json!({"value": value, "display": value.filter(|v| v.is_finite()).map_or_else(|| "N/A".into(), |v| format!("{v:.1}")), "unit":unit,"detail":detail});
        let profile = s
            .and_then(|s| s.epp_profiles.first())
            .map(|(_, p)| p.as_str())
            .filter(|p| s.is_some_and(|s| s.epp_profiles.iter().all(|(_, other)| other == p)))
            .unwrap_or("mixed");
        let channels: Vec<Value> = s
            .and_then(|s| s.memory.as_ref())
            .map(|m| {
                m.channels
                    .iter()
                    .map(|c| channel(c, &m.mem_type.to_string(), self.baseline.as_ref()))
                    .collect()
            })
            .unwrap_or_default();
        let cores: Vec<Value> = m.map(|m| m.per_core.iter().map(|c| json!({"id":c.core_id, "clock":display(c.frequency_mhz,0), "temperature":display(c.temp_c,1), "power":display(c.power_w,1), "activity": c.c0_pct.or(c.activity_pct),"sleep":c.cc6_pct.or(c.sleep_pct)})).collect()).unwrap_or_default();
        let firmware = s.and_then(|s| s.firmware.as_ref());
        let fw_rows: Vec<Value> = firmware.map(|f| f.settings.iter().map(|r| json!({"name":r.name,"value":r.display,"raw":r.raw,"id":format!("0x{:08X}",r.id),"note":r.interpretation,"electrical":r.interpretation.starts_with("Provisional") || r.interpretation.starts_with("Firmware setting")})).collect()).unwrap_or_default();
        let differences = self
            .baseline
            .as_ref()
            .zip(s)
            .map(|(a, b)| monitor::compare(a, b))
            .transpose();
        let (differences, compare_error) = match differences {
            Ok(rows) => (rows.unwrap_or_default(), None),
            Err(e) => (Vec::new(), Some(e)),
        };
        let profiles: Vec<_> = EppProfile::all().iter().enumerate().map(|(i,p)| json!({"level":i,"key":p.as_str(),"title":(["Performance","Balanced performance","Balanced power","Power saver"][i]),"description":p.description(),"active":profile == p.as_str()})).collect();
        let candidates: Vec<_> = s.map(|s| s.candidates.iter().map(|r| json!({"offset":format!("0x{:03X}",r.offset),"label":r.label,"value":r.value.map_or_else(|| "N/A".into(),|v|format!("{v:.3}"))})).collect()).unwrap_or_default();
        json!({
            "ready":s.is_some(),"demo":self.demo,"elevated":self.elevated,"authorizing":self.authorizing,"paused":self.paused,
            "message":self.message,"messageError":self.message_error,
            "cpu":s.map(|s| s.identity.model.as_str()).unwrap_or("Connecting to your hardware…"),
            "codename":s.and_then(|s|s.identity.codename).map(|c| c.as_str()).unwrap_or("CPU identification unavailable"),
            "board":s.map(|s|s.identity.board.as_str()).unwrap_or(""),"bios":s.map(|s|s.identity.bios.as_str()).unwrap_or(""),
            "physicalCores":s.and_then(|s|s.identity.topology.as_ref()).map(|t|t.physical_cores),
            "logicalCpus":s.and_then(|s|s.identity.topology.as_ref()).map(|t|t.logical_cpus),
            "timestamp":s.map(|s|s.timestamp_ms).unwrap_or(0),"memoryTimestamp":s.and_then(|s|s.memory_timestamp_ms),
            "source":m.map(|m|m.source.to_string()).unwrap_or_else(||"Waiting for a sample".into()),
            "metrics":{
                "temperature":metric(m.and_then(|m|m.tctl_temp_c),"°C","CPU control temperature"),
                "power":metric(m.and_then(|m|m.package_power_w),"W","Package power"),
                "frequency":metric(m.and_then(|m|m.peak_core_freq_mhz).map(|v|v/1000.0),"GHz","Peak sampled core clock"),
                "usage":metric(s.and_then(|s|s.cpu_usage_pct),"%","Operating system CPU usage")
            },
            "clocks":[{"name":"MCLK","value":display(m.and_then(|m|m.mclk_mhz),0),"unit":"MHz"},{"name":"UCLK","value":display(m.and_then(|m|m.uclk_mhz),0),"unit":"MHz"},{"name":"FCLK","value":display(m.and_then(|m|m.fclk_mhz),0),"unit":"MHz"},{"name":"Core voltage","value":display(m.and_then(|m|m.core_voltage_v),3),"unit":"V"},{"name":"SoC voltage","value":display(m.and_then(|m|m.soc_voltage_v),3),"unit":"V"}],
            "limits":[{"name":"PPT","value":display(m.and_then(|m|m.ppt_current_w),1),"limit":display(m.and_then(|m|m.ppt_limit_w),1),"unit":"W"},{"name":"TDC","value":display(m.and_then(|m|m.tdc_current_a),1),"limit":display(m.and_then(|m|m.tdc_limit_a),1),"unit":"A"},{"name":"EDC","value":display(m.and_then(|m|m.edc_current_a),1),"limit":display(m.and_then(|m|m.edc_limit_a),1),"unit":"A"}],
            "cores":cores,"channels":channels,"memoryError":s.and_then(|s|s.memory_error.as_ref()),
            "firmware":{"loaded":firmware.is_some(),"source":firmware.map(|f|f.source.as_str()).unwrap_or("No firmware capture imported"),"metadataMatches":firmware.is_some_and(|f|f.metadata_verified),"rows":fw_rows,"error":s.and_then(|s|s.firmware_error.as_ref())},
            "profiles":profiles,"activeProfile":profile,"eppAvailable":s.is_some_and(|s|!s.epp_profiles.is_empty()),"eppError":s.and_then(|s|s.epp_error.as_ref()),
            "baseline":{"loaded":self.baseline.is_some(),"timestamp":self.baseline.as_ref().map(|s|s.timestamp_ms),"cpu":self.baseline.as_ref().map(|s|s.identity.model.as_str())},
            "differences":differences,"compareError":compare_error,"history":self.history,"candidates":candidates,
            "pmVersion":s.and_then(|s|s.pm_table_version).map(|v|format!("0x{v:06X}"))
        }).to_string()
    }
}

fn display(value: Option<f64>, precision: usize) -> String {
    value
        .filter(|v| v.is_finite())
        .map_or_else(|| "N/A".into(), |v| format!("{v:.precision$}"))
}
fn channel(
    channel: &zentools::smu::mem::MemChannel,
    kind: &str,
    baseline: Option<&Snapshot>,
) -> Value {
    let t = &channel.timings;
    let fields = serde_json::to_value(t).unwrap_or_default();
    let groups: Vec<_> = [
        ("Primary", "tcl:tCL trcdrd:tRCDRD trcdwr:tRCDWR trp:tRP tras:tRAS trc:tRC"),
        ("Secondary & refresh", "trrds:tRRDS trrdl:tRRDL tfaw:tFAW twtrs:tWTRS twtrl:tWTRL twr:tWR tcwl:tCWL trtp:tRTP trefi:tREFI trfc:tRFC trfc2:tRFC2 trfc4:tRFC4 trfcsb:tRFCsb"),
        ("Tertiary", "trdrdscl:tRDRDSCL twrwrscl:tWRWRSCL trdrdsc:tRDRDSC trdrdsd:tRDRDSD trdrddd:tRDRDDD twrwrsc:tWRWRSC twrwrsd:tWRWRSD twrwrdd:tWRWRDD trdwr:tRDWR twrrd:tWRRD"),
        ("Additional subtimings", "trcpage:tRCPage tmod:tMOD tmodpda:tMODPDA tmrd:tMRD tmrdpda:tMRDPDA txs:tXS tdllk:tDLLK tstag:tSTAG tcke:tCKE tcpded:tCPDED txp:tXP tphywrd:tPHYWRD tphyrdl:tPHYRDL tphywrl:tPHYWRL trddataen:tRDDATAEN")
    ].iter().map(|(name,list)| {
        let rows: Vec<_> = list.split_whitespace().filter_map(|entry| {
            let (key,label) = entry.split_once(':')?;
            if (kind == "DDR5" && key == "trfc4") || (kind != "DDR5" && key == "trfcsb") { return None; }
            Some(json!({"name":label,"value":fields.get(key).and_then(Value::as_u64).map_or_else(|| "N/A".into(),|v|v.to_string())}))
        }).collect(); json!({"title":name,"rows":rows})
    }).collect();
    let old = baseline.and_then(|s| s.memory.as_ref()).and_then(|m| {
        m.channels
            .iter()
            .find(|c| c.channel_id == channel.channel_id)
    });
    let registers: Vec<_> = channel.registers.iter().map(|(&address,&value)| json!({"address":format!("0x{:08X}",(channel.channel_id<<20)|address),"value":value.map_or_else(||"N/A".into(),|v|format!("0x{v:08X}")),"changed":old.is_some_and(|c|c.registers.get(&address).copied().flatten() != value)})).collect();
    json!({"id":channel.channel_id,"label":format!("Channel {}",channel.channel_id),"type":kind,"speed":format!("{:.0}",t.frequency_mhz),"mclk":format!("{:.0}",t.frequency_mhz/2.0),"primaries":format!("{} / {} / {} / {}",t.tcl,t.trcdrd,t.trp,t.tras),"nitro":t.nitro.map_or_else(||"N/A".into(),|v|format!("{} / {} / {}",v[0],v[1],v[2])),"gdm":if t.gdm {"Enabled"} else {"Disabled"},"commandRate":if t.cmd2t {"2T"} else {"1T"},"powerDown":t.power_down.map_or("N/A",|v|if v {"Enabled"}else{"Disabled"}),"groups":groups,"registers":registers})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presentation_retains_valid_zero_and_refresh_layout() {
        let mut state = State {
            demo: true,
            ..State::default()
        };
        let mut sample = crate::demo::snapshot(0);
        sample.metrics.package_power_w = Some(0.0);
        state.accept(sample);
        let view: Value = serde_json::from_str(&state.render()).unwrap();
        assert_eq!(view["metrics"]["power"]["display"], "0.0");
        let rows = view["channels"][0]["groups"][1]["rows"].as_array().unwrap();
        assert_eq!(
            rows.iter().find(|r| r["name"] == "tRFC").unwrap()["value"],
            "884"
        );
        assert!(!rows.iter().any(|r| r["name"] == "tRFC4"));
        assert_eq!(view["firmware"]["rows"].as_array().unwrap().len(), 81);
    }
    #[test]
    fn history_is_bounded_and_baseline_survives_later_samples() {
        let mut state = State::default();
        let mut sample = crate::demo::snapshot(0);
        state.accept(sample.clone());
        state.baseline = state.latest.clone();
        for i in 1..=130 {
            sample.metrics.package_power_w = Some(i as f64);
            state.accept(sample.clone());
        }
        assert_eq!(state.history.len(), 120);
        assert_ne!(
            state.baseline.as_ref().unwrap().metrics.package_power_w,
            state.latest.as_ref().unwrap().metrics.package_power_w
        );
    }
    #[test]
    fn empty_state_does_not_invent_zero_sensor_readings() {
        let state: Value = serde_json::from_str(&State::default().render()).unwrap();
        assert!(state["metrics"]["power"]["value"].is_null());
        assert_eq!(state["metrics"]["power"]["display"], "N/A");
        assert_eq!(state["ready"], false);
    }
}
