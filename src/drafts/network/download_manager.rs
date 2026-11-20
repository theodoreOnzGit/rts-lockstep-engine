//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 20,000 programs (test: tests/differential_draft_dl.rs; the numeric DOWNLOADEVENT_*/DOWNLOADSTATUS_*
//! values are stand-ins, see the test);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Include/GameNetwork/DownloadManager.h
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/DownloadManager.cpp
//!
//! `DownloadManager`: the queue of patch downloads and the state (done / ok / error, status and error text) that
//! the download menu shows. The FTP engine it drives (`WWDownload`'s `CDownload`) is not part of this port's
//! scope here: `DownloadFile`, `PumpMessages` and `GetLastLocalFile` are the [`DownloadHost`], and the engine's
//! `IDownload` callbacks (`OnError`, `OnEnd`, ...) are methods of [`DownloadManager`] for the host to call.
//! `WWDownload/downloadDefs.h` (the numeric `DOWNLOADEVENT_*` / `DOWNLOADSTATUS_*`) is not in the repository, so
//! they are the enums [`DownloadError`] and [`DownloadStatus`]. The Winsock start-up (`WSAStartup(2.2)`) is the
//! host's.

use std::collections::VecDeque;

use crate::common::ascii_string::AsciiString;
use crate::logic::modules::plant;

/// `class QueuedDownload` (DownloadManager.h).
#[derive(Clone, Debug, Default)]
pub struct QueuedDownload {
    /// `server`.
    pub server: AsciiString,
    /// `userName`.
    pub user_name: AsciiString,
    /// `password`.
    pub password: AsciiString,
    /// `file`.
    pub file: AsciiString,
    /// `localFile`.
    pub local_file: AsciiString,
    /// `regKey`.
    pub reg_key: AsciiString,
    /// `tryResume`.
    pub try_resume: bool,
}

/// The `DOWNLOADEVENT_*` error codes `OnError` maps; anything else is `Unknown`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadError {
    /// `DOWNLOADEVENT_NOSUCHSERVER`.
    NoSuchServer,
    /// `DOWNLOADEVENT_COULDNOTCONNECT`.
    CouldNotConnect,
    /// `DOWNLOADEVENT_LOGINFAILED`.
    LoginFailed,
    /// `DOWNLOADEVENT_NOSUCHFILE`.
    NoSuchFile,
    /// `DOWNLOADEVENT_LOCALFILEOPENFAILED`.
    LocalFileOpenFailed,
    /// `DOWNLOADEVENT_TCPERROR`.
    TcpError,
    /// `DOWNLOADEVENT_DISCONNECTERROR`.
    DisconnectError,
    /// Any other code.
    Unknown,
}

/// The `DOWNLOADSTATUS_*` codes `OnStatusUpdate` maps; anything else is `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadStatus {
    /// `DOWNLOADSTATUS_CONNECTING`.
    Connecting,
    /// `DOWNLOADSTATUS_LOGGINGIN`.
    LoggingIn,
    /// `DOWNLOADSTATUS_FINDINGFILE`.
    FindingFile,
    /// `DOWNLOADSTATUS_QUERYINGRESUME`.
    QueryingResume,
    /// `DOWNLOADSTATUS_DOWNLOADING`.
    Downloading,
    /// `DOWNLOADSTATUS_DISCONNECTING`.
    Disconnecting,
    /// `DOWNLOADSTATUS_FINISHING`.
    Finishing,
    /// `DOWNLOADSTATUS_DONE`.
    Done,
    /// Any other code.
    None,
}

/// `HRESULT` of the download engine, reduced to ok / not ok (`S_OK`).
pub type HResult = i32;
/// `S_OK`.
pub const S_OK: HResult = 0;

/// The FTP engine and the game text.
pub trait DownloadHost {
    /// `TheGameText->fetch(label)`.
    fn game_text_fetch(&self, label: &[u8]) -> Vec<u16>;
    /// `CDownload::PumpMessages()`.
    fn pump_messages(&mut self) -> HResult;
    /// `CDownload::DownloadFile(server, user, pass, file, localfile, regkey, tryResume)`.
    fn download_file(&mut self, server: &[u8], username: &[u8], password: &[u8], file: &[u8], localfile: &[u8], regkey: &[u8], try_resume: bool) -> HResult;
    /// `CDownload::GetLastLocalFile(buf, 256)`.
    fn get_last_local_file(&self) -> Vec<u8>;
}

/// `class DownloadManager`.
#[derive(Clone, Debug)]
pub struct DownloadManager {
    /// `m_wasError`.
    was_error: bool,
    /// `m_sawEnd`.
    saw_end: bool,
    /// `m_errorString`.
    error_string: Vec<u16>,
    /// `m_statusString`.
    status_string: Vec<u16>,
    /// `m_queuedDownloads`.
    queued_downloads: VecDeque<QueuedDownload>,
}

impl DownloadManager {
    /// `DownloadManager::DownloadManager()`: no error, nothing seen, an empty queue, and the status text
    /// `FTP:StatusIdle`.
    pub fn new<H: DownloadHost>(host: &H) -> DownloadManager {
        DownloadManager {
            was_error: false,
            saw_end: false,
            error_string: Vec::new(),
            status_string: host.game_text_fetch(b"FTP:StatusIdle"),
            queued_downloads: VecDeque::new(),
        }
    }

    /// `init` (empty upstream).
    pub fn init(&mut self) {}
    /// `reset` (empty upstream).
    pub fn reset(&mut self) {}

    /// `update`: `m_download->PumpMessages()`.
    pub fn update<H: DownloadHost>(&mut self, host: &mut H) -> HResult {
        host.pump_messages()
    }

    /// `downloadFile`.
    pub fn download_file<H: DownloadHost>(&mut self, host: &mut H, q: &QueuedDownload) -> HResult {
        host.download_file(
            q.server.str(),
            q.user_name.str(),
            q.password.str(),
            q.file.str(),
            q.local_file.str(),
            q.reg_key.str(),
            q.try_resume,
        )
    }

    /// `queueFileForDownload`: appends the download to the queue.
    pub fn queue_file_for_download(&mut self, q: QueuedDownload) {
        self.queued_downloads.push_back(q);
    }

    /// `isFileQueuedForDownload`.
    pub fn is_file_queued_for_download(&self) -> bool {
        !self.queued_downloads.is_empty()
    }

    /// `downloadNextQueuedFile`: pops the front, clears the error/end flags and starts it; `S_OK` (with a
    /// `DEBUG_CRASH("Starting non-existent download!")`) if the queue is empty.
    pub fn download_next_queued_file<H: DownloadHost>(&mut self, host: &mut H) -> HResult {
        if let Some(q) = if plant(20305) { self.queued_downloads.pop_back() } else { self.queued_downloads.pop_front() } {
            self.was_error = false;
            self.saw_end = plant(20306) && self.saw_end;
            self.download_file(host, &q)
        } else {
            S_OK
        }
    }

    /// `getLastLocalFile`.
    pub fn get_last_local_file<H: DownloadHost>(&self, host: &H) -> AsciiString {
        AsciiString::try_from_bytes(&host.get_last_local_file()).unwrap_or_default()
    }

    /// `isDone`: the end was seen or an error happened.
    pub fn is_done(&self) -> bool {
        if plant(20307) {
            self.saw_end && self.was_error
        } else {
            self.saw_end || self.was_error
        }
    }
    /// `isOk`.
    pub fn is_ok(&self) -> bool {
        self.saw_end
    }
    /// `wasError`.
    pub fn was_error(&self) -> bool {
        self.was_error
    }
    /// `getStatusString`.
    pub fn get_status_string(&self) -> Vec<u16> {
        self.status_string.clone()
    }
    /// `getErrorString`.
    pub fn get_error_string(&self) -> Vec<u16> {
        self.error_string.clone()
    }

    /// `OnError`: flags the error and fetches the matching `FTP:` text (`FTP:UnknownError` for any other code).
    pub fn on_error<H: DownloadHost>(&mut self, host: &H, error: DownloadError) -> HResult {
        self.was_error = !plant(20301);
        let s: &[u8] = match error {
            DownloadError::NoSuchServer => b"FTP:NoSuchServer",
            DownloadError::CouldNotConnect => b"FTP:CouldNotConnect",
            DownloadError::LoginFailed => b"FTP:LoginFailed",
            DownloadError::NoSuchFile => b"FTP:NoSuchFile",
            DownloadError::LocalFileOpenFailed => b"FTP:LocalFileOpenFailed",
            DownloadError::TcpError => if plant(20302) { &b"FTP:DisconnectError"[..] } else { &b"FTP:TCPError"[..] },
            DownloadError::DisconnectError => if plant(20302) { &b"FTP:TCPError"[..] } else { &b"FTP:DisconnectError"[..] },
            DownloadError::Unknown => b"FTP:UnknownError",
        };
        if !plant(20303) {
            self.error_string = host.game_text_fetch(s);
        }
        S_OK
    }

    /// `OnEnd`: the download finished.
    pub fn on_end(&mut self) -> HResult {
        self.saw_end = !plant(20304);
        S_OK
    }

    /// `OnQueryResume`: answers `DOWNLOADEVENT_RESUME` (the do-not-resume return is commented out upstream);
    /// returns true for "resume".
    pub fn on_query_resume(&self) -> bool {
        !plant(20308)
    }

    /// `OnProgressUpdate`: only debug logging upstream.
    pub fn on_progress_update(&self, _bytesread: i32, _totalsize: i32, _timetaken: i32, _timeleft: i32) -> HResult {
        S_OK
    }

    /// `OnStatusUpdate`: fetches the `FTP:Status*` text for the status (`FTP:StatusNone` otherwise).
    pub fn on_status_update<H: DownloadHost>(&mut self, host: &H, status: DownloadStatus) -> HResult {
        let s: &[u8] = match status {
            DownloadStatus::Connecting => b"FTP:StatusConnecting",
            DownloadStatus::LoggingIn => b"FTP:StatusLoggingIn",
            DownloadStatus::FindingFile => b"FTP:StatusFindingFile",
            DownloadStatus::QueryingResume => b"FTP:StatusQueryingResume",
            DownloadStatus::Downloading => b"FTP:StatusDownloading",
            DownloadStatus::Disconnecting => b"FTP:StatusDisconnecting",
            DownloadStatus::Finishing => b"FTP:StatusFinishing",
            DownloadStatus::Done => b"FTP:StatusDone",
            DownloadStatus::None => b"FTP:StatusNone",
        };
        self.status_string = host.game_text_fetch(s);
        S_OK
    }
}
