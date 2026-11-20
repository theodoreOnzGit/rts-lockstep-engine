//! DIFFERENTIAL-TESTED (#202, #215): the ten path helpers, `get_base_path_from_path` to `get_readme_from_map`, match the upstream C++ on 60,000 strings each (test:
//! tests/differential_draft_ft_paths.rs); `do_file_transfer` and `do_any_map_transfers` match it on 20,000 scenarios, as logs of every call the transfer makes to the
//! game, the network, the shell, the load screen and the clock (after one draft fix, 2026-11-20: the timeout test and the seconds left are UNSIGNED, as upstream's
//! `UnsignedInt startTime` makes them) (test: tests/differential_draft_ftxfer.rs);
//! not yet wired into the game.
//! `get_base_path_from_path` duplicates the verified `crate::common::game_info::base_path_from_path`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngine/Source/GameNetwork/FileTransfer.cpp
//!
//! The host's map transfer: the host sends each file of a map (preview, INI, STR, solo INI, asset usage, readme, the map
//! itself) to the players that lack it, with a progress screen. `crate::common::game_info::base_path_from_path` is
//! `GetBasePathFromPath`; the rest of the path helpers are drafted here because that module has only the paths `setMap`
//! probes. `Sleep(500)` and `timeGetTime()` are the host's.

use crate::drafts::network::net_command_msg::MAX_SLOTS;
use crate::logic::modules::plant;

/// What the transfer asks of the game.
pub trait FileTransferHost {
    /// `TheGameInfo->amIHost()`.
    fn am_i_host(&self) -> bool;
    /// `TheGameInfo->getConstSlot(i)->isHuman()`.
    fn slot_is_human(&self, i: usize) -> bool;
    /// `TheGameInfo->getConstSlot(i)->hasMap()`.
    fn slot_has_map(&self, i: usize) -> bool;
    /// `TheGameInfo->getMapContentsMask()`.
    fn map_contents_mask(&self) -> i32;
    /// `game->getMap()`.
    fn map(&self) -> Vec<u8>;
    /// `Sleep(ms)`.
    fn sleep(&mut self, ms: u32);
    /// `timeGetTime()`.
    fn time_get_time(&self) -> u32;
    /// `TheNetwork->sendFileAnnounce(filename, mask)`.
    fn network_send_file_announce(&mut self, filename: &[u8], mask: u8) -> u16;
    /// `TheNetwork->areAllQueuesEmpty()`.
    fn network_are_all_queues_empty(&self) -> bool;
    /// `TheNetwork->sendFile(filename, mask, commandID)`.
    fn network_send_file(&mut self, filename: &[u8], mask: u8, command_id: u16);
    /// `TheNetwork->getFileTransferProgress(slot, filename)`.
    fn network_get_file_transfer_progress(&mut self, slot: usize, filename: &[u8]) -> i32;
    /// `ls->setCurrentFilename(filename)`.
    fn ls_set_current_filename(&mut self, filename: &[u8]);
    /// `ls->processTimeout(seconds)`.
    fn ls_process_timeout(&mut self, seconds: i32);
    /// `ls->update(percent)`.
    fn ls_update(&mut self, percent: i32);
    /// `ls->processProgress(playerId, percentage, stateStr)`.
    fn ls_process_progress(&mut self, player_id: i32, percentage: i32, state_str: &[u8]);
    /// `TheShell->hideShell()`.
    fn shell_hide(&mut self);
    /// `TheShell->showShell()`.
    fn shell_show(&mut self);
    /// `ls = NEW MapTransferLoadScreen; ls->init(TheGameInfo)`.
    fn ls_create(&mut self);
    /// `delete ls`.
    fn ls_destroy(&mut self);
}

/// `doFileTransfer(filename, ls, mask)` (FileTransfer.cpp:43): sends one file to the players in the mask and shows
/// the progress until every one of them has it or two minutes pass. Returns false on the timeout.
fn do_file_transfer<H: FileTransferHost>(h: &mut H, filename: &[u8], mask: i32) -> bool {
    let mut file_transfer_done;
    let mut file_transfer_percent;
    if mask != 0 {
        h.ls_set_current_filename(filename);
        let start_time = h.time_get_time();
        const TIMEOUT_PERIOD: i32 = 2 * 60 * 1000;
        h.ls_process_timeout(TIMEOUT_PERIOD / 1000);
        h.ls_update(0);
        file_transfer_done = false;
        file_transfer_percent = 0;
        let mut file_command_id: u16 = 0;
        let mut sent_file = false;
        if h.am_i_host() {
            if !plant(15877) {
                h.sleep(500);
            }
            file_command_id = h.network_send_file_announce(filename, mask as u8);
        } else {
            sent_file = true;
        }
        // DEBUG_LOG(("Starting file transfer loop\n"));
        while !file_transfer_done {
            if (plant(15882) || !sent_file) && h.network_are_all_queues_empty() {
                h.network_send_file(filename, mask as u8, file_command_id);
                sent_file = true;
            }
            file_transfer_done = true;
            file_transfer_percent = 100;
            for i in (if plant(15875) { 0 } else { 1 })..(MAX_SLOTS as usize) {
                if h.slot_is_human(i) && !h.slot_has_map(i) {
                    let slot_transfer_percent = h.network_get_file_transfer_progress(i, filename);
                    file_transfer_percent = if plant(15881) { std::cmp::max(file_transfer_percent, slot_transfer_percent) } else { std::cmp::min(file_transfer_percent, slot_transfer_percent) };
                    if slot_transfer_percent == 0 {
                        h.ls_process_progress(i as i32, slot_transfer_percent, b"MapTransfer:Preparing");
                    } else if slot_transfer_percent < (if plant(15876) { 101 } else { 100 }) {
                        h.ls_process_progress(i as i32, slot_transfer_percent, b"MapTransfer:Recieving");
                    } else {
                        h.ls_process_progress(i as i32, slot_transfer_percent, b"MapTransfer:Done");
                    }
                }
            }
            if file_transfer_percent < 100 {
                file_transfer_done = false;
                if file_transfer_percent == 0 {
                    h.ls_process_progress(0, file_transfer_percent, b"MapTransfer:Preparing");
                } else {
                    h.ls_process_progress(0, file_transfer_percent, b"MapTransfer:Sending");
                }
            } else {
                // DEBUG_LOG(("File transfer is 100%%!\n"));
                h.ls_process_progress(0, file_transfer_percent, b"MapTransfer:Done");
            }
            // `Int now = timeGetTime();` then `now > startTime + timeoutPeriod` with `startTime` an UnsignedInt: the Int is converted to
            // unsigned for the comparison and for `(startTime + timeoutPeriod - now)/1000` (~~compared as i32~~ **Corrected 2026-11-19
            // (#215, batch guilogic4):** the differential test found the signed compare timing out differently from upstream for clock
            // values from 2^31 ms on, and across the 2^32 wrap).
            let now = h.time_get_time() as i32;
            let deadline = start_time.wrapping_add(TIMEOUT_PERIOD as u32);
            if (if plant(15878) { now as u32 >= deadline } else { now as u32 > deadline }) {
                // bail if we don't finish in a reasonable amount of time
                break;
            } else {
                h.ls_process_timeout((deadline.wrapping_sub(now as u32) / 1000) as i32);
            }
            h.ls_update(file_transfer_percent);
        }
        if !file_transfer_done {
            return false;
        }
    }
    true
}

/// `GetBasePathFromPath(path)` (FileTransfer.cpp:155): up to the last `\`, or empty.
pub fn get_base_path_from_path(path: &[u8]) -> Vec<u8> {
    let at = if plant(12281) { path.iter().position(|&c| c == b'\\') } else { path.iter().rposition(|&c| c == b'\\') };
    match at {
        Some(len) => path[..if plant(12282) { len + 1 } else { len }].to_vec(),
        None => Vec::new(),
    }
}

/// `GetFileFromPath(path)` (:171): after the last `\`, else the whole path.
pub fn get_file_from_path(path: &[u8]) -> Vec<u8> {
    match path.iter().rposition(|&c| c == b'\\') {
        Some(i) => path[if plant(12283) { i } else { i + 1 }..].to_vec(),
        None => path.to_vec(),
    }
}

/// `GetExtensionFromFile(fname)` (:181): after the last `.`, else the whole name (sic).
pub fn get_extension_from_file(fname: &[u8]) -> Vec<u8> {
    match if plant(12290) { fname.iter().position(|&c| c == b'.') } else { fname.iter().rposition(|&c| c == b'.') } {
        Some(i) => fname[i + 1..].to_vec(),
        None => if plant(12284) { Vec::new() } else { fname.to_vec() },
    }
}

/// `GetBaseFileFromFile(fname)` (:191): up to the last `.`, or empty.
pub fn get_base_file_from_file(fname: &[u8]) -> Vec<u8> {
    match fname.iter().rposition(|&c| c == b'.') {
        Some(len) => fname[..len].to_vec(),
        None => if plant(12285) { fname.to_vec() } else { Vec::new() },
    }
}

fn join(base: &[u8], tail: &[u8]) -> Vec<u8> {
    // out.format("%s\\%s", base.str(), tail)
    let mut v = base.to_vec();
    v.push(if plant(12286) { b'/' } else { b'\\' });
    v.extend_from_slice(tail);
    v
}

/// `GetPreviewFromMap(path)` (:211): `<base>\<name>.tga`.
pub fn get_preview_from_map(path: &[u8]) -> Vec<u8> {
    let fname = if plant(12287) { get_file_from_path(path) } else { get_base_file_from_file(&get_file_from_path(path)) };
    let base = get_base_path_from_path(path);
    let mut tail = fname;
    tail.extend_from_slice(b".tga");
    join(&base, &tail)
}
/// `GetINIFromMap(path)` (:222): `<base>\map.ini`.
pub fn get_ini_from_map(path: &[u8]) -> Vec<u8> {
    join(&get_base_path_from_path(path), b"map.ini")
}
/// `GetStrFileFromMap(path)` (:231): `<base>\map.str`.
pub fn get_str_file_from_map(path: &[u8]) -> Vec<u8> {
    join(&get_base_path_from_path(path), b"map.str")
}
/// `GetSoloINIFromMap(path)` (:240): `<base>\solo.ini`.
pub fn get_solo_ini_from_map(path: &[u8]) -> Vec<u8> {
    join(&get_base_path_from_path(path), if plant(12288) { b"map.ini" } else { b"solo.ini" })
}
/// `GetAssetUsageFromMap(path)` (:249): `<base>\assetusage.txt`.
pub fn get_asset_usage_from_map(path: &[u8]) -> Vec<u8> {
    join(&get_base_path_from_path(path), b"assetusage.txt")
}
/// `GetReadmeFromMap(path)` (:258): `<base>\readme.txt`.
pub fn get_readme_from_map(path: &[u8]) -> Vec<u8> {
    join(&get_base_path_from_path(path), if plant(12289) { b"assetusage.txt" } else { b"readme.txt" })
}

/// `DoAnyMapTransfers(game)` (:267): the players (slots 1 and up) that are human and lack the map make the mask; if
/// there are none, nothing is done. Otherwise the shell hides, the transfer screen shows, each file the map's contents
/// mask names (bits 2, 4, 8, 16, 32, 64) is sent, then the map; a failure shows the shell again. Returns whether it
/// went well.
pub fn do_any_map_transfers<H: FileTransferHost>(h: &mut H) -> bool {
    let mut mask: i32 = 0;
    for i in 1..(MAX_SLOTS as usize) {
        if h.slot_is_human(i) && !h.slot_has_map(i) {
            // DEBUG_LOG(("Adding player %d to transfer mask\n", i));
            mask |= if plant(15874) { 1 << (i - 1) } else { 1 << i };
        }
    }
    if mask == 0 {
        return true;
    }
    h.shell_hide();
    h.ls_create();
    let mut ok = true;
    let contents = h.map_contents_mask();
    let map = h.map();
    if contents & 2 != 0 {
        ok = do_file_transfer(h, &get_preview_from_map(&map), mask);
    }
    if ok && contents & 4 != 0 {
        ok = do_file_transfer(h, &get_ini_from_map(&map), mask);
    }
    if ok && contents & (if plant(15879) { 4 } else { 8 }) != 0 {
        ok = do_file_transfer(h, &get_str_file_from_map(&map), mask);
    }
    if ok && contents & 16 != 0 {
        ok = do_file_transfer(h, &get_solo_ini_from_map(&map), mask);
    }
    if ok && contents & 32 != 0 {
        ok = do_file_transfer(h, &get_asset_usage_from_map(&map), mask);
    }
    if ok && contents & 64 != 0 {
        ok = do_file_transfer(h, &get_readme_from_map(&map), mask);
    }
    if ok {
        ok = do_file_transfer(h, &map, mask);
    }
    h.ls_destroy();
    if !ok || plant(15880) {
        h.shell_show();
    }
    ok
}
