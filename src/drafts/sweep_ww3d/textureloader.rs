//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/textureloader.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/textureloader.h
//!
//! The texture load queues and the load-task state machine. Upstream's tasks are an intrusive doubly-linked list with
//! tasks pooled on three free lists and a background thread. Here the tasks live in an arena ([`TextureLoaderState::tasks`],
//! a [`TaskId`] is the index), a list is a `VecDeque<TaskId>` and a task's `list` field is the membership upstream gets
//! from `Next/Prev/List`. The FastCriticalSection locks are not translated (a single owner holds the state; a threaded
//! caller wraps it in `Arc<RwLock<..>>` per the workspace rule), with a comment where upstream takes one.
//! Everything that is a Direct3D call, a DDS/Targa read, `BitmapHandlerClass` or a texture accessor is the
//! [`TextureLoaderHost`] contract.
//!
//! Already translated, so not repeated: `Validate_Texture_Size` (`client/src/drafts/terrain/w3d_common.rs`), `Init`,
//! `Deinit`, `Update`, `Remove`, `Is_Empty`, `Create`, `Destroy`, `Init`/`Deinit` of the task.
//! Not translated: the commented-out (`/* ... */`) first `Begin_Compressed_Load`/`Begin_Uncompressed_Load` at
//! textureloader.cpp:1635 (dead code upstream), and `Load_Compressed_Texture` (marked "TODO: Legacy - remove this call!").
//! Note: the cube map's `Begin_Compressed_Load` has no reduction clamp (the 2D one does); kept as upstream has it.

use std::collections::VecDeque;

use crate::formats::texture::format::Ww3dFormat;

/// `MIP_LEVELS_MAX` (texturefilter.h): `MIP_LEVELS_ALL`=0, 1..8, 10, 11, 12, then the max marker at 12.
pub const MIP_LEVELS_MAX: usize = 12;
/// `MIP_LEVELS_1`.
pub const MIP_LEVELS_1: u32 = 1;
/// `MIP_LEVELS_ALL`.
pub const MIP_LEVELS_ALL: u32 = 0;

/// Index of a task in [`TextureLoaderState::tasks`].
pub type TaskId = usize;

/// `TextureLoadTaskClass::TaskType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskType {
    None,
    Thumbnail,
    Load,
}

/// `TextureLoadTaskClass::PriorityType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorityType {
    High,
    Low,
}

/// `TextureLoadTaskClass::StateType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StateType {
    None,
    LoadBegun,
    LoadMipmap,
    LoadComplete,
    Complete,
}

/// Which of the three task classes a task is (`TextureLoadTaskClass`, `CubeTextureLoadTaskClass`,
/// `VolumeTextureLoadTaskClass`): enum dispatch in place of upstream's virtuals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskKind {
    Regular,
    Cube,
    Volume,
}

/// Which list a task is on (upstream's `TextureLoadTaskClass::List` pointer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListId {
    Foreground,
    Background,
    TexFree,
    CubeFree,
    VolFree,
}

/// A locked surface: a host handle to the pixel memory and its row pitch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LockedSurface {
    pub ptr: Option<usize>,
    pub pitch: u32,
    pub slice_pitch: u32,
}

/// `TextureLoadTaskClass` and its two subclasses' data.
#[derive(Clone, Debug)]
pub struct TextureLoadTask {
    pub kind: TaskKind,
    /// `Texture`: host handle of the owning `TextureBaseClass`.
    pub texture: Option<usize>,
    /// `D3DTexture`: host handle.
    pub d3d_texture: Option<usize>,
    pub format: Ww3dFormat,
    pub width: u32,
    pub height: u32,
    /// `VolumeTextureLoadTaskClass::Depth`.
    pub depth: u32,
    pub mip_level_count: u32,
    pub reduction: u32,
    pub ty: TaskType,
    pub priority: PriorityType,
    pub state: StateType,
    pub hsv_shift: [f32; 3],
    /// `List`: `None` is `Next == NULL && Prev == NULL`.
    pub list: Option<ListId>,
    pub locked_surface: [LockedSurface; MIP_LEVELS_MAX],
    /// `LockedCubeSurfacePtr/Pitch[6][MIP_LEVELS_MAX]`.
    pub locked_cube_surface: [[LockedSurface; MIP_LEVELS_MAX]; 6],
}

impl TextureLoadTask {
    /// `TextureLoadTaskClass::TextureLoadTaskClass()` (and the cube/volume constructors, which zero their own tables).
    pub fn new(kind: TaskKind) -> Self {
        TextureLoadTask {
            kind,
            texture: None,
            d3d_texture: None,
            format: Ww3dFormat::Unknown,
            width: 0,
            height: 0,
            depth: 0,
            mip_level_count: 0,
            reduction: 0,
            ty: TaskType::None,
            priority: PriorityType::Low,
            state: StateType::None,
            hsv_shift: [0.0; 3],
            list: None,
            locked_surface: [LockedSurface::default(); MIP_LEVELS_MAX],
            locked_cube_surface: [[LockedSurface::default(); MIP_LEVELS_MAX]; 6],
        }
    }
}

/// `TextureLoadTaskListClass` / `SynchronizedTextureLoadTaskListClass`: the queue of one [`ListId`]. The
/// Synchronized variant differs only by taking a `FastCriticalSectionClass` lock around each call (not translated).
#[derive(Clone, Debug)]
pub struct TextureLoadTaskList {
    pub id: ListId,
    pub items: VecDeque<TaskId>,
}

impl TextureLoadTaskList {
    pub fn new(id: ListId) -> Self {
        TextureLoadTaskList { id, items: VecDeque::new() }
    }

    /// `TextureLoadTaskListClass::Is_Empty`.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// `TextureLoadTaskListClass::Push_Front`: the task must not be on any list.
    pub fn push_front(&mut self, tasks: &mut [TextureLoadTask], task: TaskId) {
        debug_assert!(tasks[task].list.is_none());
        tasks[task].list = Some(self.id);
        self.items.push_front(task);
    }

    /// `TextureLoadTaskListClass::Push_Back` (textureloader.cpp:101); also
    /// `SynchronizedTextureLoadTaskListClass::Push_Back` (:179) and `::Push_Front` (:173), which add the lock.
    pub fn push_back(&mut self, tasks: &mut [TextureLoadTask], task: TaskId) {
        debug_assert!(tasks[task].list.is_none());
        tasks[task].list = Some(self.id);
        self.items.push_back(task);
    }

    /// `TextureLoadTaskListClass::Pop_Front` (:116); `SynchronizedTextureLoadTaskListClass::Pop_Front` (:185) tests
    /// emptiness before taking the lock "to save an unnecessary lock", which is the same early exit.
    pub fn pop_front(&mut self, tasks: &mut [TextureLoadTask]) -> Option<TaskId> {
        if self.is_empty() {
            return None;
        }
        let task = self.items.pop_front()?;
        tasks[task].list = None;
        Some(task)
    }

    /// `TextureLoadTaskListClass::Pop_Back` (:130); `SynchronizedTextureLoadTaskListClass::Pop_Back` (:197).
    pub fn pop_back(&mut self, tasks: &mut [TextureLoadTask]) -> Option<TaskId> {
        if self.is_empty() {
            return None;
        }
        let task = self.items.pop_back()?;
        tasks[task].list = None;
        Some(task)
    }

    /// `TextureLoadTaskListClass::Remove`: a task that is not on this list is ignored.
    pub fn remove(&mut self, tasks: &mut [TextureLoadTask], task: TaskId) {
        if tasks[task].list != Some(self.id) {
            return;
        }
        if let Some(pos) = self.items.iter().position(|&t| t == task) {
            self.items.remove(pos);
        }
        tasks[task].list = None;
    }
}

/// What `Get_Texture_Information` (a file-static of textureloader.cpp) returns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureInformation {
    pub reduction: u32,
    pub w: u32,
    pub h: u32,
    pub d: u32,
    pub format: Ww3dFormat,
    pub mip_count: u32,
}

/// Everything the loader calls outside textureloader.cpp. Texture handles are `usize`.
pub trait TextureLoaderHost {
    // upstream: ThreadClass::_Get_Current_Thread_ID() == DX8Wrapper::_Get_Main_Thread_ID()
    fn is_dx8_thread(&self) -> bool;
    // --- TextureBaseClass accessors ---
    fn tex_asset_type(&self, tc: usize) -> TaskKind;
    fn tex_full_path(&self, tc: usize) -> String;
    fn tex_mip_level_count(&self, tc: usize) -> u32;
    fn tex_reduction(&self, tc: usize) -> u32;
    fn tex_hsv_shift(&self, tc: usize) -> [f32; 3];
    fn tex_format(&self, tc: usize) -> Ww3dFormat;
    fn tex_is_compression_allowed(&self, tc: usize) -> bool;
    fn tex_is_reducible(&self, tc: usize) -> bool;
    fn tex_is_initialized(&self, tc: usize) -> bool;
    fn tex_has_d3d_texture(&self, tc: usize) -> bool;
    /// `Apply_New_Surface(d3d_texture, initialize)`.
    fn tex_apply_new_surface(&mut self, tc: usize, d3d_texture: usize, initialize: bool);
    // --- thumbnails / bitmaps ---
    /// `ThumbnailManagerClass::Peek_Thumbnail_Instance_From_Any_Manager`: width, height, original width, height, mip count, format.
    fn thumbnail_info(&self, filename: &str) -> Option<ThumbnailInfo>;
    // --- DX8 ---
    /// `DX8Wrapper::Get_Current_Caps()->Get_DX8_Caps()`: `(MaxTextureWidth, MaxTextureHeight, MaxVolumeExtent)`.
    fn max_texture_extents(&self) -> (u32, u32, u32);
    fn get_valid_texture_format(&self, format: Ww3dFormat, allow_compression: bool) -> Ww3dFormat;
    fn create_texture(&mut self, w: u32, h: u32, format: Ww3dFormat, mips: u32) -> usize;
    fn create_cube_texture(&mut self, w: u32, h: u32, format: Ww3dFormat, mips: u32) -> usize;
    fn create_volume_texture(&mut self, w: u32, h: u32, d: u32, format: Ww3dFormat, mips: u32) -> usize;
    fn texture_level_count(&self, d3d_texture: usize) -> u32;
    fn lock_rect(&mut self, d3d_texture: usize, level: u32) -> LockedSurface;
    fn lock_cube_rect(&mut self, d3d_texture: usize, face: u32, level: u32) -> LockedSurface;
    fn lock_box(&mut self, d3d_texture: usize, level: u32) -> LockedSurface;
    fn unlock_rect(&mut self, d3d_texture: usize, level: u32);
    fn unlock_cube_rect(&mut self, d3d_texture: usize, face: u32, level: u32);
    fn unlock_box(&mut self, d3d_texture: usize, level: u32);
    fn release_texture(&mut self, d3d_texture: usize);
    /// `MissingTexture::_Get_Missing_Texture()`.
    fn missing_texture(&mut self) -> usize;
    // --- file access ---
    /// `DDSFileClass dds(filename, 0)` when available: `(w, h, d, format, mip_count)` of level 0.
    fn dds_info(&self, filename: &str) -> Option<(u32, u32, u32, Ww3dFormat, u32)>;
    /// `Targa.Open` header: `(width, height, source format)`; `None` on a Targa error.
    fn targa_info(&self, filename: &str) -> Option<(u32, u32, Ww3dFormat)>;
    /// `DDSFileClass dds(path, reduction); Is_Available() && Load()`, then `Copy_Level_To_Surface` for each level.
    fn load_dds_levels(&mut self, task: &TextureLoadTask, reduction: u32) -> bool;
    /// The Targa-load and `BitmapHandlerClass::Copy_Image` mipmap generation of `Load_Uncompressed_Mipmap`.
    fn load_targa_levels(&mut self, task: &TextureLoadTask) -> bool;
    // --- WW3D statics ---
    fn ww3d_texture_reduction(&self) -> i32;
    fn ww3d_texture_min_dimension(&self) -> i32;
    // --- the queues' other collaborators ---
    /// `TextureBaseClass::Invalidate_Old_Unused_Textures(TextureInactiveOverrideTime)`.
    fn invalidate_old_unused_textures(&mut self, inactive_override_time: i32);
    fn thread_switch(&mut self);
}

/// The thumbnail fields `Get_Texture_Information` reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThumbnailInfo {
    pub original_width: u32,
    pub original_height: u32,
    pub original_mip_level_count: u32,
    pub original_format: Ww3dFormat,
}

fn is_dxt(format: Ww3dFormat) -> bool {
    matches!(format, Ww3dFormat::Dxt1 | Ww3dFormat::Dxt2 | Ww3dFormat::Dxt3 | Ww3dFormat::Dxt4 | Ww3dFormat::Dxt5)
}

/// The statics of textureloader.cpp: the queues, the free lists, the arena of tasks.
#[derive(Clone, Debug)]
pub struct TextureLoaderState {
    pub tasks: Vec<TextureLoadTask>,
    pub foreground_queue: TextureLoadTaskList,
    pub background_queue: TextureLoadTaskList,
    pub tex_load_free_list: TextureLoadTaskList,
    pub cube_tex_load_free_list: TextureLoadTaskList,
    pub vol_tex_load_free_list: TextureLoadTaskList,
    /// `TextureLoader::TextureLoadSuspended`.
    pub texture_load_suspended: bool,
    /// `TextureLoader::TextureInactiveOverrideTime`.
    pub texture_inactive_override_time: i32,
    /// `ThreadClass::running` of the background thread.
    pub running: bool,
}

impl Default for TextureLoaderState {
    fn default() -> Self {
        TextureLoaderState {
            tasks: Vec::new(),
            foreground_queue: TextureLoadTaskList::new(ListId::Foreground),
            background_queue: TextureLoadTaskList::new(ListId::Background),
            tex_load_free_list: TextureLoadTaskList::new(ListId::TexFree),
            cube_tex_load_free_list: TextureLoadTaskList::new(ListId::CubeFree),
            vol_tex_load_free_list: TextureLoadTaskList::new(ListId::VolFree),
            texture_load_suspended: false,
            texture_inactive_override_time: 0,
            running: false,
        }
    }
}

impl TextureLoaderState {
    /// `TextureLoader::Is_DX8_Thread` (textureloader.cpp:348).
    pub fn is_dx8_thread<H: TextureLoaderHost>(&self, host: &H) -> bool {
        host.is_dx8_thread()
    }

    fn remove_from(&mut self, list: ListId, task: TaskId) {
        match list {
            ListId::Foreground => self.foreground_queue.remove(&mut self.tasks, task),
            ListId::Background => self.background_queue.remove(&mut self.tasks, task),
            ListId::TexFree => self.tex_load_free_list.remove(&mut self.tasks, task),
            ListId::CubeFree => self.cube_tex_load_free_list.remove(&mut self.tasks, task),
            ListId::VolFree => self.vol_tex_load_free_list.remove(&mut self.tasks, task),
        }
    }

    /// `TextureLoadTaskClass::Delete_Free_Pool` (textureloader.cpp:1080): pop and delete every pooled task of the
    /// three free lists.
    pub fn delete_free_pool(&mut self) {
        while self.tex_load_free_list.pop_front(&mut self.tasks).is_some() {}
        while self.cube_tex_load_free_list.pop_front(&mut self.tasks).is_some() {}
        while self.vol_tex_load_free_list.pop_front(&mut self.tasks).is_some() {}
    }

    /// `TextureLoader::Load_Thumbnail(const StringClass&, const Vector3&)` (textureloader.cpp:420): builds the managed
    /// thumbnail texture. The mip chain generation (`BitmapHandlerClass::Copy_Image_Generate_Mipmap`, with the HSV
    /// shift applied to the first level only) runs in the host; this keeps the lookup-and-fallback.
    /// Returns the host handle of the texture; a thumbnail that is not found gives the missing texture.
    pub fn load_thumbnail_by_name<H: TextureLoaderHost>(&mut self, host: &mut H, filename: &str) -> usize {
        debug_assert!(host.is_dx8_thread());
        if host.thumbnail_info(filename).is_none() {
            return host.missing_texture();
        }
        // upstream: _Create_DX8_Texture(..., D3DPOOL_MANAGED), LockRect per level, Copy_Image_Generate_Mipmap per level,
        // UnlockRect per level — device-only, not translated
        let info = host.thumbnail_info(filename).unwrap();
        let dest_format = host.get_valid_texture_format(Ww3dFormat::A4R4G4B4, false);
        host.create_texture(info.original_width.max(1), info.original_height.max(1), dest_format, MIP_LEVELS_ALL)
    }

    /// `TextureLoader::Load_Surface_Immediate` (textureloader.cpp:526). The compressed path asks for the DDS texture's
    /// level 0 and the Targa path converts through `BitmapHandlerClass::Copy_Image`; both are device and bitmap work, so
    /// only the format decision (`Is_Format_Compressed`) is kept: returns `true` when the compressed path is taken.
    pub fn load_surface_immediate_uses_compressed(&self, texture_format: Ww3dFormat, allow_compression: bool, supports_dxtc: bool) -> bool {
        // Is_Format_Compressed (textureloader.cpp:~140)
        let mut compressed = false;
        if texture_format != Ww3dFormat::Unknown {
            if !supports_dxtc || !allow_compression {
                debug_assert!(!is_dxt(texture_format));
            }
            if is_dxt(texture_format) {
                compressed = true;
            }
        }
        compressed |= texture_format == Ww3dFormat::Unknown && supports_dxtc && allow_compression;
        compressed
    }

    /// `TextureLoader::Request_Thumbnail` (textureloader.cpp:635).
    pub fn request_thumbnail<H: TextureLoaderHost>(&mut self, host: &mut H, tc: usize, thumbnail_task: Option<TaskId>, texture_task: Option<TaskId>) -> Option<TaskId> {
        // FastCriticalSectionClass::LockClass lock(_ForegroundCriticalSection) — not translated
        if host.tex_has_d3d_texture(tc) {
            return thumbnail_task;
        }
        let mut task = thumbnail_task;
        if host.is_dx8_thread() {
            self.load_thumbnail(host, tc);
            if let Some(t) = task {
                self.foreground_queue.remove(&mut self.tasks, t);
                self.destroy(host, t);
                task = None;
            }
            task
        } else {
            let load_task = texture_task;
            let load_state_early = match load_task {
                None => true,
                Some(l) => self.tasks[l].state < StateType::LoadMipmap,
            };
            if task.is_none() && load_state_early {
                let t = self.create(host, tc, TaskType::Thumbnail, PriorityType::Low);
                self.foreground_queue.push_back(&mut self.tasks, t);
                task = Some(t);
            }
            task
        }
    }

    /// `TextureLoader::Request_Background_Loading` (textureloader.cpp:676).
    pub fn request_background_loading<H: TextureLoaderHost>(&mut self, host: &mut H, tc: usize, texture_task: Option<TaskId>) -> Option<TaskId> {
        if host.tex_is_initialized(tc) {
            return texture_task;
        }
        if texture_task.is_some() {
            return texture_task;
        }
        let task = self.create(host, tc, TaskType::Load, PriorityType::Low);
        if host.is_dx8_thread() {
            self.begin_load_and_queue(host, task);
        } else {
            self.foreground_queue.push_back(&mut self.tasks, task);
        }
        Some(task)
    }

    /// `TextureLoader::Request_Foreground_Loading` (textureloader.cpp:707).
    pub fn request_foreground_loading<H: TextureLoaderHost>(&mut self, host: &mut H, tc: usize, texture_task: Option<TaskId>, thumbnail_task: Option<TaskId>) {
        if host.tex_is_initialized(tc) {
            return;
        }
        let task = texture_task;
        let task_thumb = thumbnail_task;
        if host.is_dx8_thread() {
            if let Some(tt) = task_thumb {
                self.foreground_queue.remove(&mut self.tasks, tt);
                self.destroy(host, tt);
            }
            let task = match task {
                Some(t) => {
                    // _BackgroundCriticalSection held here upstream, so the background thread cannot start mipmap levels
                    self.foreground_queue.remove(&mut self.tasks, t);
                    self.background_queue.remove(&mut self.tasks, t);
                    t
                }
                None => self.create(host, tc, TaskType::Load, PriorityType::High),
            };
            self.finish_load(host, task);
            self.destroy(host, task);
        } else {
            if let Some(tt) = task_thumb {
                // leave it queued in the completed state so Update() destroys it
                self.tasks[tt].state = StateType::Complete;
            }
            match task {
                Some(t) => {
                    if self.tasks[t].list == Some(ListId::Background) {
                        self.background_queue.remove(&mut self.tasks, t);
                        self.foreground_queue.push_back(&mut self.tasks, t);
                    }
                    self.tasks[t].priority = PriorityType::High;
                }
                None => {
                    let t = self.create(host, tc, TaskType::Load, PriorityType::High);
                    self.foreground_queue.push_back(&mut self.tasks, t);
                }
            }
        }
    }

    /// `TextureLoader::Flush_Pending_Load_Tasks` (textureloader.cpp:797): loop `Update()` until both queues are empty.
    /// `update` is `TextureLoader::Update` (already translated) supplied by the caller.
    pub fn flush_pending_load_tasks<H: TextureLoaderHost>(&mut self, host: &mut H, mut update: impl FnMut(&mut Self, &mut H)) {
        debug_assert!(host.is_dx8_thread());
        loop {
            let done = self.background_queue.is_empty() && self.foreground_queue.is_empty();
            if done {
                break;
            }
            update(self, host);
            host.thread_switch();
        }
    }

    /// `TextureLoader::Suspend_Texture_Load` (textureloader.cpp:882).
    pub fn suspend_texture_load<H: TextureLoaderHost>(&mut self, host: &H) {
        debug_assert!(host.is_dx8_thread());
        self.texture_load_suspended = true;
    }

    /// `TextureLoader::Continue_Texture_Load` (textureloader.cpp:888).
    pub fn continue_texture_load<H: TextureLoaderHost>(&mut self, host: &H) {
        debug_assert!(host.is_dx8_thread());
        self.texture_load_suspended = false;
    }

    /// `TextureLoader::Process_Foreground_Thumbnail` (textureloader.cpp:894): `STATE_NONE` loads and falls through to
    /// destroying; `STATE_COMPLETE` destroys.
    pub fn process_foreground_thumbnail<H: TextureLoaderHost>(&mut self, host: &mut H, task: TaskId) {
        match self.tasks[task].state {
            StateType::None => {
                let tc = self.tasks[task].texture.unwrap_or(0);
                self.load_thumbnail(host, tc);
                self.destroy(host, task);
            }
            StateType::Complete => self.destroy(host, task),
            _ => {}
        }
    }

    /// `TextureLoader::Process_Foreground_Load` (textureloader.cpp:908).
    pub fn process_foreground_load<H: TextureLoaderHost>(&mut self, host: &mut H, task: TaskId) {
        if self.tasks[task].priority == PriorityType::High {
            self.finish_load(host, task);
            self.destroy(host, task);
            return;
        }
        match self.tasks[task].state {
            StateType::None => self.begin_load_and_queue(host, task),
            StateType::LoadMipmap => {
                self.end_load(host, task);
                self.destroy(host, task);
            }
            _ => {}
        }
    }

    /// `TextureLoader::Begin_Load_And_Queue` (textureloader.cpp:932): on success the task goes on the FRONT of the
    /// background queue (LIFO service); otherwise the missing texture is applied.
    pub fn begin_load_and_queue<H: TextureLoaderHost>(&mut self, host: &mut H, task: TaskId) {
        debug_assert!(host.is_dx8_thread());
        if self.begin_load(host, task) {
            self.background_queue.push_front(&mut self.tasks, task);
        } else {
            self.apply_missing_texture(host, task);
            self.destroy(host, task);
        }
    }

    /// `TextureLoader::Load_Thumbnail(TextureBaseClass*)` (textureloader.cpp:956).
    pub fn load_thumbnail<H: TextureLoaderHost>(&mut self, host: &mut H, tc: usize) {
        debug_assert!(host.is_dx8_thread());
        let path = host.tex_full_path(tc);
        let d3d_texture = self.load_thumbnail_by_name(host, &path);
        if host.tex_asset_type(tc) == TaskKind::Regular {
            host.tex_apply_new_surface(tc, d3d_texture, false);
        }
        host.release_texture(d3d_texture);
    }

    // ---- task lifecycle (Create/Destroy/Init/Deinit exist in the port; the bodies the drafts below call) ----

    /// `TextureLoadTaskClass::Create` (textureloader.cpp:~1040): pop from the matching free list or allocate.
    pub fn create<H: TextureLoaderHost>(&mut self, host: &mut H, tc: usize, ty: TaskType, priority: PriorityType) -> TaskId {
        let kind = host.tex_asset_type(tc);
        let popped = match kind {
            TaskKind::Regular => self.tex_load_free_list.pop_front(&mut self.tasks),
            TaskKind::Cube => self.cube_tex_load_free_list.pop_front(&mut self.tasks),
            TaskKind::Volume => self.vol_tex_load_free_list.pop_front(&mut self.tasks),
        };
        let id = match popped {
            Some(id) => id,
            None => {
                self.tasks.push(TextureLoadTask::new(kind));
                self.tasks.len() - 1
            }
        };
        self.init(host, id, tc, ty, priority);
        id
    }

    fn init<H: TextureLoaderHost>(&mut self, host: &H, id: TaskId, tc: usize, ty: TaskType, priority: PriorityType) {
        let t = &mut self.tasks[id];
        t.texture = Some(tc);
        t.ty = ty;
        t.priority = priority;
        t.state = StateType::None;
        t.d3d_texture = None;
        t.format = host.tex_format(tc);
        t.width = 0;
        t.height = 0;
        t.mip_level_count = host.tex_mip_level_count(tc);
        t.reduction = host.tex_reduction(tc);
        t.hsv_shift = host.tex_hsv_shift(tc);
        t.locked_surface = [LockedSurface::default(); MIP_LEVELS_MAX];
    }

    /// `TextureLoadTaskClass::Destroy` / `CubeTextureLoadTaskClass::Destroy` / `VolumeTextureLoadTaskClass::Destroy`:
    /// detach, then return to the free list of the task's own class.
    pub fn destroy<H: TextureLoaderHost>(&mut self, _host: &mut H, id: TaskId) {
        self.tasks[id].texture = None;
        self.tasks[id].ty = TaskType::None;
        match self.tasks[id].kind {
            TaskKind::Regular => self.tex_load_free_list.push_front(&mut self.tasks, id),
            TaskKind::Cube => self.cube_tex_load_free_list.push_front(&mut self.tasks, id),
            TaskKind::Volume => self.vol_tex_load_free_list.push_front(&mut self.tasks, id),
        }
    }

    /// `TextureLoadTaskClass::Begin_Load`: compressed first when allowed, then uncompressed, then lock the surfaces.
    pub fn begin_load<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId) -> bool {
        debug_assert!(host.is_dx8_thread());
        let tc = self.tasks[id].texture.unwrap_or(0);
        let mut loaded = false;
        if host.tex_is_compression_allowed(tc) {
            loaded = self.begin_compressed_load(host, id);
        }
        if !loaded {
            loaded = self.begin_uncompressed_load(host, id);
        }
        if !loaded {
            return false;
        }
        self.lock_surfaces(host, id);
        self.tasks[id].state = StateType::LoadBegun;
        true
    }

    /// `TextureLoadTaskClass::Load`: mipmaps, compressed first when allowed.
    pub fn load<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId) -> bool {
        let tc = self.tasks[id].texture.unwrap_or(0);
        let mut loaded = false;
        if host.tex_is_compression_allowed(tc) {
            loaded = self.load_compressed_mipmap(host, id);
        }
        if !loaded {
            loaded = self.load_uncompressed_mipmap(host, id);
        }
        self.tasks[id].state = StateType::LoadMipmap;
        loaded
    }

    /// `TextureLoadTaskClass::End_Load` (textureloader.cpp:1243): unlock, apply with `initialize = true`.
    pub fn end_load<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId) {
        debug_assert!(host.is_dx8_thread());
        self.unlock_surfaces(host, id);
        self.apply(host, id, true);
        self.tasks[id].state = StateType::LoadComplete;
    }

    /// `TextureLoadTaskClass::Finish_Load`: the switch falls through on purpose (NONE -> BEGUN -> MIPMAP).
    pub fn finish_load<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId) {
        let mut state = self.tasks[id].state;
        if state == StateType::None {
            if !self.begin_load(host, id) {
                self.apply_missing_texture(host, id);
                return;
            }
            state = StateType::LoadBegun;
        }
        if state == StateType::LoadBegun {
            self.load(host, id);
            state = StateType::LoadMipmap;
        }
        if state == StateType::LoadMipmap {
            self.end_load(host, id);
        }
    }

    /// `TextureLoadTaskClass::Apply_Missing_Texture` (textureloader.cpp:1277).
    pub fn apply_missing_texture<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId) {
        debug_assert!(host.is_dx8_thread());
        debug_assert!(self.tasks[id].d3d_texture.is_none());
        self.tasks[id].d3d_texture = Some(host.missing_texture());
        self.apply(host, id, true);
    }

    /// `TextureLoadTaskClass::Apply`: no mip level may still be locked.
    pub fn apply<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId, initialize: bool) {
        let d3d = self.tasks[id].d3d_texture;
        debug_assert!(d3d.is_some());
        for i in 0..self.tasks[id].mip_level_count as usize {
            debug_assert!(self.tasks[id].locked_surface[i].ptr.is_none());
        }
        if let (Some(tc), Some(d3d)) = (self.tasks[id].texture, d3d) {
            host.tex_apply_new_surface(tc, d3d, initialize);
            host.release_texture(d3d);
        }
        self.tasks[id].d3d_texture = None;
    }

    /// `Get_Texture_Information` (file-static, textureloader.cpp): size, format, mips and the clamped reduction for a file.
    pub fn get_texture_information<H: TextureLoaderHost>(&self, host: &H, filename: &str, reduction_in: u32, compressed: bool) -> Option<TextureInformation> {
        let mut reduction = reduction_in;
        match host.thumbnail_info(filename) {
            None => {
                let (w, h, d, format, mip_count);
                if compressed {
                    let (dw, dh, dd, dformat, dmips) = host.dds_info(filename)?;
                    w = dw;
                    h = dh;
                    d = dd;
                    format = dformat;
                    mip_count = dmips;
                } else {
                    let (tw, th, tformat) = host.targa_info(filename)?;
                    // count the mip levels the texture will occupy
                    let mut mips = 0u32;
                    let (mut i, mut j) = (tw as i32, th as i32);
                    while i > 0 && j > 0 {
                        mips += 1;
                        i >>= 1;
                        j >>= 1;
                    }
                    w = tw;
                    h = th;
                    d = 1;
                    format = tformat;
                    mip_count = mips;
                }
                let (cw, ch) = (w as i32, h as i32);
                reduction = Self::clamp_reduction(host, mip_count, cw, ch);
                Some(TextureInformation { reduction, w, h, d, format, mip_count })
            }
            Some(thumb) => {
                if compressed && !is_dxt(thumb.original_format) {
                    return None;
                }
                let w = thumb.original_width >> reduction;
                let h = thumb.original_height >> reduction;
                Some(TextureInformation { reduction, w, h, d: 1, format: thumb.original_format, mip_count: thumb.original_mip_level_count })
            }
        }
    }

    /// The reduction clamp both branches of `Get_Texture_Information` share: the requested reduction, held below the mip
    /// count and stopped before a side falls to `WW3D::Get_Texture_Min_Dimension()`.
    fn clamp_reduction<H: TextureLoaderHost>(host: &H, mip_count: u32, width: i32, height: i32) -> u32 {
        let mut req_reduction = host.ww3d_texture_reduction();
        if req_reduction >= mip_count as i32 {
            req_reduction = mip_count as i32 - 1;
        }
        let mut cur_reduction = 0;
        let mut cur_width = width;
        let mut cur_height = height;
        let min_dim = host.ww3d_texture_min_dimension();
        while cur_reduction < req_reduction && cur_width > min_dim && cur_height > min_dim {
            cur_width >>= 1;
            cur_height >>= 1;
            cur_reduction += 1;
        }
        cur_reduction as u32
    }

    /// Hardware-limit size validation used by the three `Begin_*` functions: `TextureLoader::Validate_Texture_Size`
    /// (already translated in `client/src/drafts/terrain/w3d_common.rs`), reproduced here only to keep this file free of
    /// a client-crate dependency. Rounds to powers of two, caps at the device limits, and caps the aspect ratio at 8:1.
    fn validate_texture_size<H: TextureLoaderHost>(host: &H, width: &mut u32, height: &mut u32, depth: &mut u32) {
        let (max_w, max_h, max_d) = host.max_texture_extents();
        let mut pw = 1u32;
        while pw < *width {
            pw <<= 1;
        }
        let mut ph = 1u32;
        while ph < *height {
            ph <<= 1;
        }
        let mut pd = 1u32;
        while pd < *depth {
            pd <<= 1;
        }
        if pw > max_w {
            pw = max_w;
        }
        if ph > max_h {
            ph = max_h;
        }
        if pd > max_d {
            pd = max_d;
        }
        if pw > ph {
            while pw / ph > 8 {
                ph *= 2;
            }
        } else {
            while ph / pw > 8 {
                pw *= 2;
            }
        }
        *width = pw;
        *height = ph;
        *depth = pd;
    }

    /// The common reduction fix-up of `TextureLoadTaskClass::Begin_Compressed_Load` and `Begin_Uncompressed_Load`.
    fn adjust_reduction<H: TextureLoaderHost>(host: &H, tc: usize, reduction_in: u32, orig_mip_count: u32) -> u32 {
        let mut reduction = reduction_in;
        let tex_mips = host.tex_mip_level_count(tc);
        if !host.tex_is_reducible(tc) || tex_mips == MIP_LEVELS_1 {
            reduction = 0;
        } else if tex_mips != MIP_LEVELS_ALL && (tex_mips as i64 - reduction as i64) < 1 {
            reduction = tex_mips - 1;
        }
        if reduction >= orig_mip_count {
            reduction = 0;
        }
        reduction
    }

    /// `max_mip_level_count` loop at the end of each `Begin_Compressed_Load`.
    fn max_mip_levels(width: u32, height: u32) -> u32 {
        let mut max_mip_level_count = 1;
        let mut w = 4u32;
        let mut h = 4u32;
        while w < width && h < height {
            w += w;
            h += h;
            max_mip_level_count += 1;
        }
        max_mip_level_count
    }

    /// The "texture too big for the hardware: try a smaller mip" search of the three compressed loads.
    fn search_smaller_mip<H: TextureLoaderHost>(host: &H, orig_w: u32, orig_h: u32, orig_d: u32, orig_mip_count: u32, use_depth: bool) -> Option<(u32, u32, u32, u32)> {
        for i in 1..orig_mip_count {
            let mut w = orig_w >> i;
            if w < 4 {
                w = 4;
            }
            let mut h = orig_h >> i;
            if h < 4 {
                h = 4;
            }
            let mut d = if use_depth { orig_d >> i } else { orig_d };
            if use_depth && d < 1 {
                d = 1;
            }
            let (tmp_w, tmp_h, tmp_d) = (w, h, d);
            Self::validate_texture_size(host, &mut w, &mut h, &mut d);
            if w == tmp_w && h == tmp_h && (!use_depth || d == tmp_d) {
                return Some((i, w, h, d));
            }
        }
        None
    }

    /// `TextureLoadTaskClass::Begin_Compressed_Load` (textureloader.cpp:1410), `kind == Regular`;
    /// `CubeTextureLoadTaskClass::Begin_Compressed_Load` (:2221) and `VolumeTextureLoadTaskClass::Begin_Compressed_Load`
    /// (:2581) are the other arms.
    pub fn begin_compressed_load<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId) -> bool {
        let tc = self.tasks[id].texture.unwrap_or(0);
        let kind = self.tasks[id].kind;
        let path = host.tex_full_path(tc);
        let info = match self.get_texture_information(host, &path, self.tasks[id].reduction, true) {
            Some(i) => i,
            None => return false,
        };
        let (orig_w, orig_h, orig_d, orig_mip_count) = (info.w, info.h, info.d, info.mip_count);
        let mut width = orig_w;
        let mut height = orig_h;
        let mut depth = orig_d;
        Self::validate_texture_size(host, &mut width, &mut height, &mut depth);
        let use_depth = kind == TaskKind::Volume;
        let mismatch = width != orig_w || height != orig_h || (use_depth && depth != orig_d);
        if mismatch {
            if let Some((i, w, h, d)) = Self::search_smaller_mip(host, orig_w, orig_h, orig_d, orig_mip_count, use_depth) {
                self.tasks[id].reduction += i;
                width = w;
                height = h;
                if use_depth {
                    depth = d;
                }
            }
        }
        let allow = host.tex_is_compression_allowed(tc);
        self.tasks[id].width = width;
        self.tasks[id].height = height;
        if use_depth {
            self.tasks[id].depth = depth;
        }
        self.tasks[id].format = host.get_valid_texture_format(info.format, allow);

        let mut mip_level_count = self.tasks[id].mip_level_count;
        let (mut reduced_w, mut reduced_h) = (width, height);
        if kind == TaskKind::Regular {
            self.tasks[id].reduction = info.reduction;
            self.tasks[id].reduction = Self::adjust_reduction(host, tc, info.reduction, orig_mip_count);
            let reduction = self.tasks[id].reduction;
            if mip_level_count == 0 {
                reduced_w >>= reduction;
                reduced_h >>= reduction;
                mip_level_count = orig_mip_count - reduction;
                if mip_level_count < 1 {
                    mip_level_count = 1;
                }
            } else {
                if mip_level_count > orig_mip_count {
                    mip_level_count = orig_mip_count;
                }
                if reduction != 0 {
                    reduced_w >>= reduction;
                    reduced_h >>= reduction;
                    mip_level_count -= reduction;
                }
            }
        } else if mip_level_count == 0 {
            // cube and volume have no reduction clamp
            mip_level_count = orig_mip_count;
        } else if mip_level_count > orig_mip_count {
            mip_level_count = orig_mip_count;
        }
        let max_mip_level_count = Self::max_mip_levels(width, height);
        if mip_level_count > max_mip_level_count {
            mip_level_count = max_mip_level_count;
        }
        let format = self.tasks[id].format;
        let created = match kind {
            TaskKind::Regular => host.create_texture(reduced_w, reduced_h, format, mip_level_count),
            TaskKind::Cube => host.create_cube_texture(width, height, format, mip_level_count),
            TaskKind::Volume => host.create_volume_texture(width, height, depth, format, mip_level_count),
        };
        self.tasks[id].d3d_texture = Some(created);
        self.tasks[id].mip_level_count = mip_level_count;
        true
    }

    /// `TextureLoadTaskClass::Begin_Uncompressed_Load` (the live one, textureloader.cpp:~1560) for the Regular arm; the
    /// cube (:2323) and volume (:2689) arms differ only in the create call and in not clamping the reduction.
    pub fn begin_uncompressed_load<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId) -> bool {
        let tc = self.tasks[id].texture.unwrap_or(0);
        let kind = self.tasks[id].kind;
        let path = host.tex_full_path(tc);
        let info = match self.get_texture_information(host, &path, self.tasks[id].reduction, false) {
            Some(i) => i,
            None => return false,
        };
        let src_format = info.format;
        let dest_format = host.get_valid_texture_format(src_format, false);
        // upstream: WWDEBUG_SAY("Invalid TGA format used in %s ...") when not A8R8G8B8/R8G8B8/X8R8G8B8 — debug log, not translated
        let (mut width, mut height, mut depth) = (info.w, info.h, info.d);
        Self::validate_texture_size(host, &mut width, &mut height, &mut depth);
        self.tasks[id].width = width;
        self.tasks[id].height = height;
        if kind == TaskKind::Volume {
            self.tasks[id].depth = depth;
        }
        if kind == TaskKind::Regular {
            self.tasks[id].reduction = Self::adjust_reduction(host, tc, info.reduction, info.mip_count);
        }
        if self.tasks[id].format == Ww3dFormat::Unknown {
            self.tasks[id].format = dest_format;
        } else {
            let f = self.tasks[id].format;
            self.tasks[id].format = host.get_valid_texture_format(f, false);
        }
        let format = self.tasks[id].format;
        let tex_mips = host.tex_mip_level_count(tc);
        let created = match kind {
            TaskKind::Regular => {
                let reduction = self.tasks[id].reduction;
                let (mut rw, mut rh, mut rm) = (width, height, tex_mips);
                if reduction != 0 {
                    rw >>= reduction;
                    rh >>= reduction;
                    if rm != MIP_LEVELS_ALL {
                        rm -= reduction;
                    }
                }
                host.create_texture(rw, rh, format, rm)
            }
            TaskKind::Cube => host.create_cube_texture(width, height, format, tex_mips),
            TaskKind::Volume => host.create_volume_texture(width, height, depth, format, tex_mips),
        };
        self.tasks[id].d3d_texture = Some(created);
        true
    }

    /// `TextureLoadTaskClass::Lock_Surfaces` (textureloader.cpp:1779), `CubeTextureLoadTaskClass::Lock_Surfaces` (:2160),
    /// `VolumeTextureLoadTaskClass::Lock_Surfaces` (:2532).
    pub fn lock_surfaces<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId) {
        let d3d = match self.tasks[id].d3d_texture {
            Some(d) => d,
            None => return,
        };
        match self.tasks[id].kind {
            TaskKind::Regular => {
                self.tasks[id].mip_level_count = host.texture_level_count(d3d);
                for i in 0..self.tasks[id].mip_level_count {
                    self.tasks[id].locked_surface[i as usize] = host.lock_rect(d3d, i);
                }
            }
            TaskKind::Cube => {
                for f in 0..6u32 {
                    for i in 0..self.tasks[id].mip_level_count {
                        self.tasks[id].locked_cube_surface[f as usize][i as usize] = host.lock_cube_rect(d3d, f, i);
                    }
                }
            }
            TaskKind::Volume => {
                for i in 0..self.tasks[id].mip_level_count {
                    self.tasks[id].locked_surface[i as usize] = host.lock_box(d3d, i);
                }
            }
        }
    }

    /// `TextureLoadTaskClass::Unlock_Surfaces` (textureloader.cpp:1802), `CubeTextureLoadTaskClass::Unlock_Surfaces`
    /// (:2184), `VolumeTextureLoadTaskClass::Unlock_Surfaces` (:2554). The `#ifndef USE_MANAGED_TEXTURES` copy to a
    /// `D3DPOOL_DEFAULT` texture is compiled out (`USE_MANAGED_TEXTURES` is defined).
    pub fn unlock_surfaces<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId) {
        let d3d = self.tasks[id].d3d_texture;
        let kind = self.tasks[id].kind;
        let mips = self.tasks[id].mip_level_count;
        match kind {
            TaskKind::Cube => {
                for f in 0..6usize {
                    for i in 0..mips as usize {
                        if self.tasks[id].locked_cube_surface[f][i].ptr.is_some() {
                            debug_assert!(host.is_dx8_thread());
                            if let Some(d) = d3d {
                                host.unlock_cube_rect(d, f as u32, i as u32);
                            }
                        }
                        self.tasks[id].locked_cube_surface[f][i].ptr = None;
                    }
                }
            }
            _ => {
                for i in 0..mips as usize {
                    if self.tasks[id].locked_surface[i].ptr.is_some() {
                        debug_assert!(host.is_dx8_thread());
                        if let Some(d) = d3d {
                            if kind == TaskKind::Volume {
                                host.unlock_box(d, i as u32);
                            } else {
                                host.unlock_rect(d, i as u32);
                            }
                        }
                    }
                    self.tasks[id].locked_surface[i].ptr = None;
                }
            }
        }
    }

    /// `TextureLoadTaskClass::Load_Compressed_Mipmap` (textureloader.cpp:1825); the Cube (:2391) and Volume (:2760) arms
    /// loop faces / depth the same way. The `DDSFileClass::Copy_Level_To_Surface` copies are the host's; the per-level
    /// size walk (halve each level, volume clamps to 1) is here.
    pub fn load_compressed_mipmap<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId) -> bool {
        let task = self.tasks[id].clone();
        if !host.load_dds_levels(&task, task.reduction) {
            return false;
        }
        match task.kind {
            TaskKind::Regular => {
                let mut width = task.width;
                let mut height = task.height;
                for _ in 0..task.reduction {
                    width >>= 1;
                    height >>= 1;
                }
                for level in 0..task.mip_level_count {
                    debug_assert!(width != 0 && height != 0);
                    let _ = self.get_locked_surface_ptr(id, level);
                    let _ = self.get_locked_surface_pitch(id, level);
                    width >>= 1;
                    height >>= 1;
                }
            }
            TaskKind::Cube => {
                for face in 0..6u32 {
                    let mut width = task.width;
                    let mut height = task.height;
                    for level in 0..task.mip_level_count {
                        debug_assert!(width != 0 && height != 0);
                        let _ = self.get_locked_cube_map_surface_pointer(id, face, level);
                        let _ = self.get_locked_cube_map_surface_pitch(id, face, level);
                        width >>= 1;
                        height >>= 1;
                    }
                }
            }
            TaskKind::Volume => {
                let mut depth = task.depth;
                let mut width = task.width;
                let mut height = task.height;
                debug_assert!(width != 0 && height != 0 && depth != 0);
                for level in 0..task.mip_level_count {
                    if width < 1 {
                        width = 1;
                    }
                    if height < 1 {
                        height = 1;
                    }
                    if depth < 1 {
                        depth = 1;
                    }
                    let _ = self.get_locked_volume_pointer(id, level);
                    let _ = self.get_locked_volume_row_pitch(id, level);
                    let _ = self.get_locked_volume_slice_pitch(id, level);
                    width >>= 1;
                    height >>= 1;
                    depth >>= 1;
                }
            }
        }
        true
    }

    /// `TextureLoadTaskClass::Load_Uncompressed_Mipmap` (textureloader.cpp:1868): false when there are no mip levels;
    /// the Targa open / format conversion / reduction / per-level `Copy_Image` is the host's `load_targa_levels`.
    pub fn load_uncompressed_mipmap<H: TextureLoaderHost>(&mut self, host: &mut H, id: TaskId) -> bool {
        if self.tasks[id].mip_level_count == 0 {
            return false;
        }
        let task = self.tasks[id].clone();
        host.load_targa_levels(&task)
    }

    /// `TextureLoadTaskClass::Get_Locked_Surface_Ptr` (textureloader.cpp:2012).
    pub fn get_locked_surface_ptr(&self, id: TaskId, level: u32) -> Option<usize> {
        debug_assert!(level < self.tasks[id].mip_level_count);
        debug_assert!(self.tasks[id].locked_surface[level as usize].ptr.is_some());
        self.tasks[id].locked_surface[level as usize].ptr
    }

    /// `TextureLoadTaskClass::Get_Locked_Surface_Pitch` (textureloader.cpp:2027).
    pub fn get_locked_surface_pitch(&self, id: TaskId, level: u32) -> u32 {
        debug_assert!(level < self.tasks[id].mip_level_count);
        debug_assert!(self.tasks[id].locked_surface[level as usize].ptr.is_some());
        self.tasks[id].locked_surface[level as usize].pitch
    }

    /// `CubeTextureLoadTaskClass::Get_Locked_CubeMap_Surface_Pointer` (textureloader.cpp:2432).
    pub fn get_locked_cube_map_surface_pointer(&self, id: TaskId, face: u32, level: u32) -> Option<usize> {
        debug_assert!(face < 6 && level < self.tasks[id].mip_level_count);
        debug_assert!(self.tasks[id].locked_cube_surface[face as usize][level as usize].ptr.is_some());
        self.tasks[id].locked_cube_surface[face as usize][level as usize].ptr
    }

    /// `CubeTextureLoadTaskClass::Get_Locked_CubeMap_Surface_Pitch` (textureloader.cpp:2439): asserts on the PITCH, not
    /// the pointer, as upstream does.
    pub fn get_locked_cube_map_surface_pitch(&self, id: TaskId, face: u32, level: u32) -> u32 {
        debug_assert!(face < 6 && level < self.tasks[id].mip_level_count);
        debug_assert!(self.tasks[id].locked_cube_surface[face as usize][level as usize].pitch != 0);
        self.tasks[id].locked_cube_surface[face as usize][level as usize].pitch
    }

    /// `VolumeTextureLoadTaskClass::Get_Locked_Volume_Pointer` (textureloader.cpp:2805).
    pub fn get_locked_volume_pointer(&self, id: TaskId, level: u32) -> Option<usize> {
        self.get_locked_surface_ptr(id, level)
    }

    /// `VolumeTextureLoadTaskClass::Get_Locked_Volume_Row_Pitch` (textureloader.cpp:2812).
    pub fn get_locked_volume_row_pitch(&self, id: TaskId, level: u32) -> u32 {
        self.get_locked_surface_pitch(id, level)
    }

    /// `VolumeTextureLoadTaskClass::Get_Locked_Volume_Slice_Pitch` (textureloader.cpp:2819).
    pub fn get_locked_volume_slice_pitch(&self, id: TaskId, level: u32) -> u32 {
        debug_assert!(level < self.tasks[id].mip_level_count);
        debug_assert!(self.tasks[id].locked_surface[level as usize].ptr.is_some());
        self.tasks[id].locked_surface[level as usize].slice_pitch
    }
}
