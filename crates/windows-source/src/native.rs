//! Windows Event Log pull subscription using `windows-rs`.
//!
//! Native handles are owned by RAII wrappers. The blocking wait and render
//! calls stay off an async executor: callers drive this from a dedicated
//! thread.

use std::ffi::c_void;
use std::mem::size_of;

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use thiserror::Error;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, SYSTEMTIME, WAIT_OBJECT_0};
use windows::Win32::System::EventLog::{
    EvtChannelLoggingConfigMaxSize, EvtClose, EvtCreateBookmark, EvtGetChannelConfigProperty,
    EvtGetEventInfo, EvtNext, EvtOpenChannelConfig, EvtOpenChannelPath, EvtOpenLog, EvtQuery,
    EvtQueryChannelPath, EvtQueryForwardDirection, EvtRender, EvtRenderBookmark, EvtRenderEventXml,
    EvtSeek, EvtSeekRelativeToFirst, EvtSubscribe, EvtSubscribeStartAfterBookmark,
    EvtSubscribeToFutureEvents, EvtSystemEventRecordId, EvtSystemTimeCreated, EvtUpdateBookmark,
    EVT_CHANNEL_CONFIG_PROPERTY_ID, EVT_EVENT_PROPERTY_ID, EVT_HANDLE, EVT_VARIANT,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

use super::health::{assess_channel, ChannelInfo};

const WAIT_TIMEOUT_VALUE: u32 = 0x102;
const DEFAULT_TIMEOUT_MS: u32 = 1000;
const MAX_BATCH: usize = 64;

/// The bindings type `EvtSystem*` constants as `EVT_SYSTEM_PROPERTY_ID`, while
/// `EvtGetEventInfo` takes `EVT_EVENT_PROPERTY_ID`. Both are i32 newtypes.
const EVENT_RECORD_ID: EVT_EVENT_PROPERTY_ID = EVT_EVENT_PROPERTY_ID(EvtSystemEventRecordId.0);
const TIME_CREATED: EVT_EVENT_PROPERTY_ID = EVT_EVENT_PROPERTY_ID(EvtSystemTimeCreated.0);

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("windows: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("invalid UTF-16: {0}")]
    Utf16(#[from] std::string::FromUtf16Error),
    #[error("invalid source state: {0}")]
    State(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedEvent {
    pub record_id: u64,
    pub xml: String,
    pub time_created: DateTime<Utc>,
}

struct OwnedEvt(EVT_HANDLE);

impl OwnedEvt {
    fn get(&self) -> EVT_HANDLE {
        self.0
    }
}

impl Drop for OwnedEvt {
    fn drop(&mut self) {
        unsafe {
            let _ = EvtClose(self.0);
        }
    }
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// A pull subscription on one channel. `next_batch` is blocking and should be
/// called from one dedicated thread.
pub struct EventLogSource {
    channel: HSTRING,
    _log: OwnedEvt,
    config: OwnedEvt,
    bookmark: OwnedEvt,
    signal: OwnedHandle,
    subscription: Option<OwnedEvt>,
}

impl EventLogSource {
    /// Open a channel and subscribe. With a bookmark, collection resumes
    /// after the bookmarked record; without one it starts at the current tail.
    pub fn open(channel: &str, bookmark_xml: Option<&str>) -> Result<Self, SourceError> {
        let channel = HSTRING::from(channel);
        let log = OwnedEvt(unsafe { EvtOpenLog(None, &channel, EvtOpenChannelPath.0)? });
        let config = OwnedEvt(unsafe { EvtOpenChannelConfig(None, &channel, 0)? });
        let bookmark = OwnedEvt(match bookmark_xml {
            Some(xml) => unsafe { EvtCreateBookmark(&HSTRING::from(xml))? },
            None => unsafe { EvtCreateBookmark(PCWSTR::null())? },
        });
        let signal = OwnedHandle(unsafe { CreateEventW(None, false, false, None)? });
        let query = HSTRING::from(super::EVENT_QUERY);

        let (subscription_bookmark, flags) = match bookmark_xml {
            Some(_) => (Some(bookmark.get()), EvtSubscribeStartAfterBookmark.0),
            None => (None, EvtSubscribeToFutureEvents.0),
        };
        let subscription = OwnedEvt(unsafe {
            EvtSubscribe(
                None,
                Some(signal.0),
                &channel,
                &query,
                subscription_bookmark,
                None,
                None,
                flags,
            )?
        });

        Ok(Self {
            channel,
            _log: log,
            config,
            bookmark,
            signal,
            subscription: Some(subscription),
        })
    }

    /// Channel size and coverage horizon used to degrade health honestly.
    pub fn channel_info(&self) -> Result<ChannelInfo, SourceError> {
        let max_size =
            unsafe { channel_config_u64(self.config.get(), EvtChannelLoggingConfigMaxSize)? };
        let oldest_time = self.oldest_record_time()?;
        Ok(ChannelInfo {
            max_size_bytes: max_size,
            oldest_record_time: oldest_time,
            observed_at: Utc::now(),
        })
    }

    /// Assess the channel against the 64 MiB and 24 hour coverage floors.
    pub fn health(&self) -> Result<super::health::ChannelHealth, SourceError> {
        Ok(assess_channel(&self.channel_info()?))
    }

    /// Wait for source activity, then render up to `max` available records.
    /// The bookmark advances only for events returned to the caller.
    pub fn next_batch(
        &mut self,
        max: usize,
        timeout_ms: u32,
    ) -> Result<Vec<RenderedEvent>, SourceError> {
        let Some(subscription) = self.subscription.as_ref() else {
            return Err(SourceError::State("subscription is closed".to_string()));
        };
        let timeout = if timeout_ms == 0 {
            DEFAULT_TIMEOUT_MS
        } else {
            timeout_ms
        };
        let wait = unsafe { WaitForSingleObject(self.signal.0, timeout) };
        if wait != WAIT_OBJECT_0 && wait.0 != WAIT_TIMEOUT_VALUE {
            return Err(SourceError::State(format!(
                "source wait failed with {:?}",
                wait
            )));
        }

        let capacity = max.clamp(1, MAX_BATCH);
        let mut handles = vec![0isize; capacity];
        let mut returned = 0u32;
        unsafe {
            EvtNext(subscription.get(), &mut handles, timeout, 0, &mut returned)?;
        }

        let mut batch = Vec::with_capacity(returned as usize);
        for handle in handles.into_iter().take(returned as usize) {
            let event = EVT_HANDLE(handle);
            let rendered = RenderedEvent {
                record_id: unsafe { event_u64(event, EVENT_RECORD_ID)? },
                xml: render(event, EvtRenderEventXml)?,
                time_created: unsafe { event_time(event, TIME_CREATED)? },
            };
            unsafe {
                EvtUpdateBookmark(self.bookmark.get(), event)?;
                EvtClose(event)?;
            }
            batch.push(rendered);
        }
        Ok(batch)
    }

    /// Persistable bookmark XML. Store it with the record in the same local
    /// transaction so a restart resumes from the same channel position.
    pub fn bookmark_xml(&self) -> Result<String, SourceError> {
        render(self.bookmark.get(), EvtRenderBookmark)
    }

    fn oldest_record_time(&self) -> Result<Option<DateTime<Utc>>, SourceError> {
        let query = HSTRING::from("*");
        let result_set = OwnedEvt(unsafe {
            EvtQuery(
                None,
                &self.channel,
                &query,
                EvtQueryChannelPath.0 | EvtQueryForwardDirection.0,
            )?
        });
        unsafe {
            EvtSeek(
                result_set.get(),
                0,
                None,
                Some(DEFAULT_TIMEOUT_MS),
                EvtSeekRelativeToFirst.0,
            )?;
        }
        let mut handles = [0isize; 1];
        let mut returned = 0u32;
        unsafe {
            EvtNext(
                result_set.get(),
                &mut handles,
                DEFAULT_TIMEOUT_MS,
                0,
                &mut returned,
            )?;
        }
        if returned == 0 {
            return Ok(None);
        }
        let event = EVT_HANDLE(handles[0]);
        let time = unsafe { event_time(event, TIME_CREATED)? };
        unsafe {
            EvtClose(event)?;
        }
        Ok(Some(time))
    }
}

unsafe fn channel_config_u64(
    config: EVT_HANDLE,
    property: EVT_CHANNEL_CONFIG_PROPERTY_ID,
) -> Result<u64, SourceError> {
    let mut value: EVT_VARIANT = std::mem::zeroed();
    let mut used = 0u32;
    unsafe {
        EvtGetChannelConfigProperty(
            config,
            property,
            0,
            size_of::<EVT_VARIANT>() as u32,
            Some(&mut value),
            &mut used,
        )?;
        Ok(value.Anonymous.UInt64Val)
    }
}

unsafe fn event_u64(
    event: EVT_HANDLE,
    property: EVT_EVENT_PROPERTY_ID,
) -> Result<u64, SourceError> {
    let mut value: EVT_VARIANT = std::mem::zeroed();
    let mut used = 0u32;
    unsafe {
        EvtGetEventInfo(
            event,
            property,
            size_of::<EVT_VARIANT>() as u32,
            Some(&mut value),
            &mut used,
        )?;
        Ok(value.Anonymous.UInt64Val)
    }
}

unsafe fn event_time(
    event: EVT_HANDLE,
    property: EVT_EVENT_PROPERTY_ID,
) -> Result<DateTime<Utc>, SourceError> {
    let mut value: EVT_VARIANT = std::mem::zeroed();
    let mut used = 0u32;
    unsafe {
        EvtGetEventInfo(
            event,
            property,
            size_of::<EVT_VARIANT>() as u32,
            Some(&mut value),
            &mut used,
        )?;
        let system_time: &SYSTEMTIME = &*value.Anonymous.SysTimeVal;
        system_time_to_utc(system_time)
            .ok_or_else(|| SourceError::State("invalid SYSTEMTIME from source".to_string()))
    }
}

fn system_time_to_utc(value: &SYSTEMTIME) -> Option<DateTime<Utc>> {
    let date = NaiveDate::from_ymd_opt(value.wYear as i32, value.wMonth as u32, value.wDay as u32)?;
    let time = NaiveTime::from_hms_milli_opt(
        value.wHour as u32,
        value.wMinute as u32,
        value.wSecond as u32,
        value.wMilliseconds as u32,
    )?;
    Some(DateTime::from_naive_utc_and_offset(
        date.and_time(time),
        Utc,
    ))
}

/// `EvtRender` returns UTF-16 in its buffer. The first call only reports the
/// required size; a short read is not a parse failure for an empty fragment.
fn render(
    fragment: EVT_HANDLE,
    flag: windows::Win32::System::EventLog::EVT_RENDER_FLAGS,
) -> Result<String, SourceError> {
    let mut used = 0u32;
    let mut count = 0u32;
    unsafe {
        let _ = EvtRender(None, fragment, flag.0, 0, None, &mut used, &mut count);
    }
    if used == 0 {
        return Ok(String::new());
    }
    let mut bytes = vec![0u8; used as usize];
    unsafe {
        EvtRender(
            None,
            fragment,
            flag.0,
            used,
            Some(bytes.as_mut_ptr() as *mut c_void),
            &mut used,
            &mut count,
        )?;
    }
    let (pairs, _) = bytes[..used as usize].as_chunks::<2>();
    let units: Vec<u16> = pairs.iter().map(|pair| u16::from_ne_bytes(*pair)).collect();
    Ok(String::from_utf16(&units)?)
}
