//! UNTESTED DRAFT (#202, #204): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DBufferManager.h
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DBufferManager.cpp
//!
//! Upstream links slots and buffers with raw pointers into fixed arrays (`m_W3DVertexBufferEmptySlots[MAX_NUMBER_SLOTS]`,
//! `m_W3DEmptyVertexBuffers`, ...). Here those arrays are `Vec`s and every pointer is an `Option<usize>` index into
//! them, so the lists, the counts and the limits are the same. The DX8 vertex and index buffers are device objects:
//! `dx8_buffer` is `true` while upstream's `m_DX8VertexBuffer` / `m_DX8IndexBuffer` would be non-NULL, and nothing is
//! created. The `DEBUG_ASSERTCRASH`es are `debug_assert!`s.

/// `MAX_VB_SIZES`: number of different sized VB slots allowed.
pub const MAX_VB_SIZES: usize = 128;
/// `MIN_SLOT_SIZE`: minimum number of vertices allocated per slot (power of 2).
pub const MIN_SLOT_SIZE: i32 = 32;
/// `MIN_SLOT_SIZE_SHIFT`: used for division by `MIN_SLOT_SIZE`.
pub const MIN_SLOT_SIZE_SHIFT: i32 = 5;
/// `MAX_VERTEX_BUFFERS_CREATED`: maximum number of D3D vertex buffers allowed to create per vertex type.
pub const MAX_VERTEX_BUFFERS_CREATED: usize = 32;
/// `DEFAULT_VERTEX_BUFFER_SIZE`: this size ends up generating VB's of about 256Kbytes.
pub const DEFAULT_VERTEX_BUFFER_SIZE: i32 = 8192;
/// `MAX_NUMBER_SLOTS`: maximum number of slots that can be allocated.
pub const MAX_NUMBER_SLOTS: usize = 4096;
/// `MAX_IB_SIZES`: number of different sized IB slots allowed (goes all the way up to 65536).
pub const MAX_IB_SIZES: usize = 128;
/// `MAX_INDEX_BUFFERS_CREATED`.
pub const MAX_INDEX_BUFFERS_CREATED: usize = 32;
/// `DEFAULT_INDEX_BUFFER_SIZE`.
pub const DEFAULT_INDEX_BUFFER_SIZE: i32 = 32768;

/// `W3DBufferManager::VBM_FVF_TYPES`: all possible vertex formats available to the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum VbmFvfTypes {
    /// `VBM_FVF_XYZ`: position.
    Xyz = 0,
    /// `VBM_FVF_XYZD`: position, diffuse.
    Xyzd,
    /// `VBM_FVF_XYZUV`: position, uv.
    Xyzuv,
    /// `VBM_FVF_XYZDUV`: position, diffuse, uv.
    Xyzduv,
    /// `VBM_FVF_XYZUV2`: position, uv1, uv2.
    Xyzuv2,
    /// `VBM_FVF_XYZDUV2`: position, diffuse, uv1, uv2.
    Xyzduv2,
    /// `VBM_FVF_XYZN`: position, normal.
    Xyzn,
    /// `VBM_FVF_XYZND`: position, normal, diffuse.
    Xyznd,
    /// `VBM_FVF_XYZNUV`: position, normal, uv.
    Xyznuv,
    /// `VBM_FVF_XYZNDUV`: position, normal, diffuse, uv.
    Xyznduv,
    /// `VBM_FVF_XYZNUV2`: position, normal, uv1, uv2.
    Xyznuv2,
    /// `VBM_FVF_XYZNDUV2`: position, normal, diffuse, uv1, uv2.
    Xyznduv2,
    /// `VBM_FVF_XYZRHW`: transformed position.
    Xyzrhw,
    /// `VBM_FVF_XYZRHWD`: transformed position, diffuse.
    Xyzrhwd,
    /// `VBM_FVF_XYZRHWUV`: transformed position, uv.
    Xyzrhwuv,
    /// `VBM_FVF_XYZRHWDUV`: transformed position, diffuse, uv.
    Xyzrhwduv,
    /// `VBM_FVF_XYZRHWUV2`: transformed position, uv1, uv2.
    Xyzrhwuv2,
    /// `VBM_FVF_XYZRHWDUV2`: transformed position, diffuse, uv1, uv2.
    Xyzrhwduv2,
}

/// `MAX_FVF`.
pub const MAX_FVF: usize = 18;

// D3D8 flexible-vertex-format bits (d3d8types.h), for `FVFTypeIndexList`.
const D3DFVF_XYZ: i32 = 0x002;
const D3DFVF_XYZRHW: i32 = 0x004;
const D3DFVF_NORMAL: i32 = 0x010;
const D3DFVF_DIFFUSE: i32 = 0x040;
const D3DFVF_TEX1: i32 = 0x100;
const D3DFVF_TEX2: i32 = 0x200;

/// `FVFTypeIndexList` (W3DBufferManager.cpp:30).
const FVF_TYPE_INDEX_LIST: [i32; MAX_FVF] = [
    D3DFVF_XYZ,
    D3DFVF_XYZ | D3DFVF_DIFFUSE,
    D3DFVF_XYZ | D3DFVF_TEX1,
    D3DFVF_XYZ | D3DFVF_DIFFUSE | D3DFVF_TEX1,
    D3DFVF_XYZ | D3DFVF_TEX2,
    D3DFVF_XYZ | D3DFVF_DIFFUSE | D3DFVF_TEX2,
    D3DFVF_XYZ | D3DFVF_NORMAL,
    D3DFVF_XYZ | D3DFVF_NORMAL | D3DFVF_DIFFUSE,
    D3DFVF_XYZ | D3DFVF_NORMAL | D3DFVF_TEX1,
    D3DFVF_XYZ | D3DFVF_NORMAL | D3DFVF_DIFFUSE | D3DFVF_TEX1,
    D3DFVF_XYZ | D3DFVF_NORMAL | D3DFVF_TEX2,
    D3DFVF_XYZ | D3DFVF_NORMAL | D3DFVF_DIFFUSE | D3DFVF_TEX2,
    D3DFVF_XYZRHW,
    D3DFVF_XYZRHW | D3DFVF_DIFFUSE,
    D3DFVF_XYZRHW | D3DFVF_TEX1,
    D3DFVF_XYZRHW | D3DFVF_DIFFUSE | D3DFVF_TEX1,
    D3DFVF_XYZRHW | D3DFVF_TEX2,
    D3DFVF_XYZRHW | D3DFVF_DIFFUSE | D3DFVF_TEX2,
];

/// `W3DBufferManager::W3DVertexBufferSlot`. Pointers are indices: `vb` into `empty_vertex_buffers`, the links into
/// `vertex_buffer_empty_slots`.
#[derive(Clone, Copy, Debug, Default)]
pub struct W3DVertexBufferSlot {
    /// `m_size`: number of vertices in slot.
    pub size: i32,
    /// `m_start`: index of first vertex within VB.
    pub start: i32,
    /// `m_VB`: vertex buffer holding this slot.
    pub vb: Option<usize>,
    /// `m_prevSameSize`.
    pub prev_same_size: Option<usize>,
    /// `m_nextSameSize`.
    pub next_same_size: Option<usize>,
    /// `m_prevSameVB`.
    pub prev_same_vb: Option<usize>,
    /// `m_nextSameVB`.
    pub next_same_vb: Option<usize>,
}

/// `W3DBufferManager::W3DVertexBuffer`.
#[derive(Clone, Copy, Debug)]
pub struct W3DVertexBuffer {
    /// `m_format`: format of vertices in this VB.
    pub format: VbmFvfTypes,
    /// `m_usedSlots`: slots inside this vertex buffer being used (head index).
    pub used_slots: Option<usize>,
    /// `m_startFreeIndex`: index of vertex at start of unallocated memory.
    pub start_free_index: i32,
    /// `m_size`: number of vertices allowed in VB.
    pub size: i32,
    /// `m_nextVB`: next vertex buffer of same type.
    pub next_vb: Option<usize>,
    /// True while upstream's `m_DX8VertexBuffer` would be non-NULL (the device buffer itself is not translated).
    pub dx8_buffer: bool,
}

impl Default for W3DVertexBuffer {
    fn default() -> Self {
        W3DVertexBuffer { format: VbmFvfTypes::Xyz, used_slots: None, start_free_index: 0, size: 0, next_vb: None, dx8_buffer: false }
    }
}

/// `W3DBufferManager::W3DIndexBufferSlot`. `ib` indexes `empty_index_buffers`, the links `index_buffer_empty_slots`.
#[derive(Clone, Copy, Debug, Default)]
pub struct W3DIndexBufferSlot {
    /// `m_size`: number of vertices in slot.
    pub size: i32,
    /// `m_start`: index of first index within VB.
    pub start: i32,
    /// `m_IB`: index buffer holding this slot.
    pub ib: Option<usize>,
    /// `m_prevSameSize`.
    pub prev_same_size: Option<usize>,
    /// `m_nextSameSize`.
    pub next_same_size: Option<usize>,
    /// `m_prevSameIB`.
    pub prev_same_ib: Option<usize>,
    /// `m_nextSameIB`.
    pub next_same_ib: Option<usize>,
}

/// `W3DBufferManager::W3DIndexBuffer`.
#[derive(Clone, Copy, Debug, Default)]
pub struct W3DIndexBuffer {
    /// `m_usedSlots`: slots inside this index buffer being used (head index).
    pub used_slots: Option<usize>,
    /// `m_startFreeIndex`: index of index at start of unallocated memory.
    pub start_free_index: i32,
    /// `m_size`: number of vertices allowed in VB.
    pub size: i32,
    /// `m_nextIB`: next index buffer of same type.
    pub next_ib: Option<usize>,
    /// True while upstream's `m_DX8IndexBuffer` would be non-NULL.
    pub dx8_buffer: bool,
}

/// `class W3DBufferManager` (W3DBufferManager.h). `TheW3DBufferManager` is the caller's to hold.
pub struct W3DBufferManager {
    /// `m_W3DVertexBufferSlots[MAX_FVF][MAX_VB_SIZES]`: previously allocated slots free to fill again (list heads).
    pub vertex_buffer_slots: Vec<Vec<Option<usize>>>,
    /// `m_W3DVertexBuffers[MAX_FVF]`: allocated vertex buffers of each type (list heads).
    pub vertex_buffers: Vec<Option<usize>>,
    /// `m_W3DVertexBufferEmptySlots[MAX_NUMBER_SLOTS]`.
    pub vertex_buffer_empty_slots: Vec<W3DVertexBufferSlot>,
    /// `m_numEmptySlotsAllocated`.
    pub num_empty_slots_allocated: i32,
    /// `m_W3DEmptyVertexBuffers[MAX_VERTEX_BUFFERS_CREATED]`.
    pub empty_vertex_buffers: Vec<W3DVertexBuffer>,
    /// `m_numEmptyVertexBuffersAllocated`.
    pub num_empty_vertex_buffers_allocated: i32,
    /// `m_W3DIndexBufferSlots[MAX_IB_SIZES]`.
    pub index_buffer_slots: Vec<Option<usize>>,
    /// `m_W3DIndexBuffers` (list head).
    pub index_buffers: Option<usize>,
    /// `m_W3DIndexBufferEmptySlots[MAX_NUMBER_SLOTS]`.
    pub index_buffer_empty_slots: Vec<W3DIndexBufferSlot>,
    /// `m_numEmptyIndexSlotsAllocated`.
    pub num_empty_index_slots_allocated: i32,
    /// `m_W3DEmptyIndexBuffers[MAX_INDEX_BUFFERS_CREATED]`.
    pub empty_index_buffers: Vec<W3DIndexBuffer>,
    /// `m_numEmptyIndexBuffersAllocated`.
    pub num_empty_index_buffers_allocated: i32,
}

impl Default for W3DBufferManager {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DBufferManager {
    /// `W3DBufferManager::getDX8Format` (W3DBufferManager.cpp:54): translates our vertex format into the D3D equivalent.
    pub fn get_dx8_format(format: VbmFvfTypes) -> i32 {
        FVF_TYPE_INDEX_LIST[format as usize]
    }

    /// `W3DBufferManager::W3DBufferManager` (W3DBufferManager.cpp:59).
    pub fn new() -> Self {
        W3DBufferManager {
            vertex_buffer_slots: vec![vec![None; MAX_VB_SIZES]; MAX_FVF],
            vertex_buffers: vec![None; MAX_FVF],
            vertex_buffer_empty_slots: vec![W3DVertexBufferSlot::default(); MAX_NUMBER_SLOTS],
            num_empty_slots_allocated: 0,
            empty_vertex_buffers: vec![W3DVertexBuffer::default(); MAX_VERTEX_BUFFERS_CREATED],
            num_empty_vertex_buffers_allocated: 0,
            index_buffer_slots: vec![None; MAX_IB_SIZES],
            index_buffers: None,
            index_buffer_empty_slots: vec![W3DIndexBufferSlot::default(); MAX_NUMBER_SLOTS],
            num_empty_index_slots_allocated: 0,
            empty_index_buffers: vec![W3DIndexBuffer::default(); MAX_INDEX_BUFFERS_CREATED],
            num_empty_index_buffers_allocated: 0,
        }
    }

    /// `W3DBufferManager::~W3DBufferManager` (W3DBufferManager.cpp:77).
    pub fn destroy(&mut self) {
        self.free_all_slots();
        self.free_all_buffers();
    }

    /// `W3DBufferManager::freeAllSlots` (W3DBufferManager.cpp:83): release all slots to pool.
    pub fn free_all_slots(&mut self) {
        for i in 0..MAX_FVF {
            for j in 0..MAX_VB_SIZES {
                // Release all slots allocated for each size
                let mut vb_slot = self.vertex_buffer_slots[i][j];
                while let Some(s) = vb_slot {
                    let slot = self.vertex_buffer_empty_slots[s];
                    if let Some(prev) = slot.prev_same_vb {
                        self.vertex_buffer_empty_slots[prev].next_same_vb = slot.next_same_vb;
                    } else if let Some(vb) = slot.vb {
                        self.empty_vertex_buffers[vb].used_slots = None;
                    }
                    if let Some(next) = slot.next_same_vb {
                        self.vertex_buffer_empty_slots[next].prev_same_vb = slot.prev_same_vb;
                    }
                    vb_slot = slot.next_same_size;
                    self.num_empty_slots_allocated -= 1;
                }
                self.vertex_buffer_slots[i][j] = None;
            }
        }

        for j in 0..MAX_IB_SIZES {
            // Release all slots allocated for each size
            let mut ib_slot = self.index_buffer_slots[j];
            while let Some(s) = ib_slot {
                let slot = self.index_buffer_empty_slots[s];
                if let Some(prev) = slot.prev_same_ib {
                    self.index_buffer_empty_slots[prev].next_same_ib = slot.next_same_ib;
                } else if let Some(ib) = slot.ib {
                    self.empty_index_buffers[ib].used_slots = None;
                }
                if let Some(next) = slot.next_same_ib {
                    self.index_buffer_empty_slots[next].prev_same_ib = slot.prev_same_ib;
                }
                ib_slot = slot.next_same_size;
                self.num_empty_index_slots_allocated -= 1;
            }
            self.index_buffer_slots[j] = None;
        }

        debug_assert!(self.num_empty_slots_allocated == 0, "Failed to free all empty vertex buffer slots");
        debug_assert!(self.num_empty_index_slots_allocated == 0, "Failed to free all empty index buffer slots");
    }

    /// `W3DBufferManager::freeAllBuffers` (W3DBufferManager.cpp:136): release all vertex buffers to pool.
    pub fn free_all_buffers(&mut self) {
        // Make sure all slots are free
        self.free_all_slots(); // release all slots to pool.

        for i in 0..MAX_FVF {
            let mut vb = self.vertex_buffers[i];
            while let Some(v) = vb {
                debug_assert!(self.empty_vertex_buffers[v].used_slots.is_none(), "Freeing Non-Empty Vertex Buffer");
                self.empty_vertex_buffers[v].dx8_buffer = false; // upstream: REF_PTR_RELEASE(vb->m_DX8VertexBuffer)
                self.num_empty_vertex_buffers_allocated -= 1;
                vb = self.empty_vertex_buffers[v].next_vb; // get next vertex buffer of this type
            }
            self.vertex_buffers[i] = None;
        }

        let mut ib = self.index_buffers;
        while let Some(b) = ib {
            debug_assert!(self.empty_index_buffers[b].used_slots.is_none(), "Freeing Non-Empty Index Buffer");
            self.empty_index_buffers[b].dx8_buffer = false; // upstream: REF_PTR_RELEASE(ib->m_DX8IndexBuffer)
            self.num_empty_index_buffers_allocated -= 1;
            ib = self.empty_index_buffers[b].next_ib; // get next vertex buffer of this type
        }
        self.index_buffers = None;

        debug_assert!(self.num_empty_vertex_buffers_allocated == 0, "Failed to free all empty vertex buffers");
        debug_assert!(self.num_empty_index_buffers_allocated == 0, "Failed to free all empty index buffers");
    }

    /// `W3DBufferManager::ReleaseResources` (W3DBufferManager.cpp:176): release D3D/W3D resources.
    pub fn release_resources(&mut self) {
        for i in 0..MAX_FVF {
            let mut vb = self.vertex_buffers[i];
            while let Some(v) = vb {
                self.empty_vertex_buffers[v].dx8_buffer = false; // upstream: REF_PTR_RELEASE(vb->m_DX8VertexBuffer)
                vb = self.empty_vertex_buffers[v].next_vb;
            }
        }

        let mut ib = self.index_buffers;
        while let Some(b) = ib {
            self.empty_index_buffers[b].dx8_buffer = false; // upstream: REF_PTR_RELEASE(ib->m_DX8IndexBuffer)
            ib = self.empty_index_buffers[b].next_ib;
        }
    }

    /// `W3DBufferManager::ReAcquireResources` (W3DBufferManager.cpp:196): reacquire D3D/W3D resources.
    pub fn re_acquire_resources(&mut self) -> bool {
        for i in 0..MAX_FVF {
            let mut vb = self.vertex_buffers[i];
            while let Some(v) = vb {
                debug_assert!(!self.empty_vertex_buffers[v].dx8_buffer, "ReAcquire of existing vertex buffer");
                // upstream: NEW_REF(DX8VertexBufferClass, (FVFTypeIndexList[format], size, USAGE_DEFAULT)) — device-only, not translated
                self.empty_vertex_buffers[v].dx8_buffer = true;
                vb = self.empty_vertex_buffers[v].next_vb;
            }
        }

        let mut ib = self.index_buffers;
        while let Some(b) = ib {
            debug_assert!(!self.empty_index_buffers[b].dx8_buffer, "ReAcquire of existing index buffer");
            // upstream: NEW_REF(DX8IndexBufferClass, (size, USAGE_DEFAULT)) — device-only, not translated
            self.empty_index_buffers[b].dx8_buffer = true;
            ib = self.empty_index_buffers[b].next_ib;
        }

        true
    }

    /// `W3DBufferManager::getSlot(VBM_FVF_TYPES, Int)` (W3DBufferManager.cpp:231): searches through previously allocated
    /// vertex buffer slots and returns a matching type. If none found, creates a new slot and adds it to the pool.
    /// Returns the slot's index, `None` on failure.
    pub fn get_slot_vb(&mut self, fvf_type: VbmFvfTypes, size: i32) -> Option<usize> {
        // round size to next multiple of minimum slot size.
        // should help avoid fragmentation.
        let size = (size + (MIN_SLOT_SIZE - 1)) & !(MIN_SLOT_SIZE - 1);
        let size_index = ((size >> MIN_SLOT_SIZE_SHIFT) - 1) as usize;

        debug_assert!(size_index < MAX_VB_SIZES && size != 0, "Allocating too large vertex buffer slot");

        if let Some(vb_slot) = self.vertex_buffer_slots[fvf_type as usize][size_index] {
            // found a previously allocated slot matching required size
            let next = self.vertex_buffer_empty_slots[vb_slot].next_same_size;
            self.vertex_buffer_slots[fvf_type as usize][size_index] = next;
            if let Some(n) = next {
                self.vertex_buffer_empty_slots[n].prev_same_size = None;
            }
            Some(vb_slot)
        } else {
            // need to allocate a new slot
            self.allocate_slot_storage_vb(fvf_type, size)
        }
    }

    /// `W3DBufferManager::releaseSlot(W3DVertexBufferSlot *)` (W3DBufferManager.cpp:262): returns vertex buffer space back
    /// to pool so it can be reused later.
    pub fn release_slot_vb(&mut self, vb_slot: usize) {
        let slot = self.vertex_buffer_empty_slots[vb_slot];
        let size_index = ((slot.size >> MIN_SLOT_SIZE_SHIFT) - 1) as usize;
        let format = self.empty_vertex_buffers[slot.vb.expect("slot has a vertex buffer")].format as usize;

        let head = self.vertex_buffer_slots[format][size_index];
        self.vertex_buffer_empty_slots[vb_slot].next_same_size = head;
        if let Some(h) = head {
            self.vertex_buffer_empty_slots[h].prev_same_size = Some(vb_slot);
        }

        self.vertex_buffer_slots[format][size_index] = Some(vb_slot);
    }

    /// `W3DBufferManager::allocateSlotStorage(VBM_FVF_TYPES, Int)` (W3DBufferManager.cpp:276): reserves space inside an
    /// existing vertex buffer or allocates a new one to fit the required size.
    pub fn allocate_slot_storage_vb(&mut self, fvf_type: VbmFvfTypes, size: i32) -> Option<usize> {
        debug_assert!((self.num_empty_slots_allocated as usize) < MAX_NUMBER_SLOTS, "Nore more VB Slots");

        let mut p_vb = self.vertex_buffers[fvf_type as usize];
        while let Some(v) = p_vb {
            if (self.empty_vertex_buffers[v].size - self.empty_vertex_buffers[v].start_free_index) >= size {
                // found enough free space in this vertex buffer
                if (self.num_empty_slots_allocated as usize) < MAX_NUMBER_SLOTS {
                    // we're allowing more slots to be allocated.
                    let s = self.num_empty_slots_allocated as usize;
                    let used = self.empty_vertex_buffers[v].used_slots;
                    let start = self.empty_vertex_buffers[v].start_free_index;
                    {
                        let vb_slot = &mut self.vertex_buffer_empty_slots[s];
                        vb_slot.size = size;
                        vb_slot.start = start;
                        vb_slot.vb = Some(v);
                        // Link to VB list of slots
                        vb_slot.next_same_vb = used;
                        vb_slot.prev_same_vb = None; // this will be the new head
                    }
                    if let Some(u) = used {
                        self.vertex_buffer_empty_slots[u].prev_same_vb = Some(s);
                    }
                    self.vertex_buffer_empty_slots[s].prev_same_size = None;
                    self.vertex_buffer_empty_slots[s].next_same_size = None;
                    self.empty_vertex_buffers[v].used_slots = Some(s);
                    self.empty_vertex_buffers[v].start_free_index += size;
                    self.num_empty_slots_allocated += 1;
                    return Some(s);
                }
            }
            p_vb = self.empty_vertex_buffers[v].next_vb;
        }

        let old_head = self.vertex_buffers[fvf_type as usize]; // save old list head

        // Didn't find any vertex buffers with room, create a new one
        debug_assert!((self.num_empty_vertex_buffers_allocated as usize) < MAX_VERTEX_BUFFERS_CREATED, "Reached Max Static VB Shadow Geometry");

        if (self.num_empty_vertex_buffers_allocated as usize) < MAX_VERTEX_BUFFERS_CREATED {
            let v = self.num_empty_vertex_buffers_allocated as usize;
            self.vertex_buffers[fvf_type as usize] = Some(v);
            self.empty_vertex_buffers[v].next_vb = old_head; // link to list
            self.num_empty_vertex_buffers_allocated += 1;

            // get new list head
            let vb_size = DEFAULT_VERTEX_BUFFER_SIZE.max(size);

            // upstream: NEW_REF(DX8VertexBufferClass, (FVFTypeIndexList[fvfType], vbSize, USAGE_DEFAULT)) — device-only, not translated
            self.empty_vertex_buffers[v].dx8_buffer = true;
            self.empty_vertex_buffers[v].format = fvf_type;
            self.empty_vertex_buffers[v].start_free_index = size;
            self.empty_vertex_buffers[v].size = vb_size;
            let s = self.num_empty_slots_allocated as usize;
            self.num_empty_slots_allocated += 1;
            self.empty_vertex_buffers[v].used_slots = Some(s);
            let vb_slot = &mut self.vertex_buffer_empty_slots[s];
            vb_slot.size = size;
            vb_slot.start = 0;
            vb_slot.vb = Some(v);
            vb_slot.prev_same_vb = None;
            vb_slot.next_same_vb = None;
            vb_slot.prev_same_size = None;
            vb_slot.next_same_size = None;
            return Some(s);
        }

        None
    }

    // ******************************** Index Buffer code ******************************************************

    /// `W3DBufferManager::getSlot(Int)` (W3DBufferManager.cpp:372): searches through previously allocated index buffer
    /// slots and returns a matching type. If none found, creates a new slot and adds it to the pool.
    pub fn get_slot_ib(&mut self, size: i32) -> Option<usize> {
        // round size to next multiple of minimum slot size.
        // should help avoid fragmentation.
        let size = (size + (MIN_SLOT_SIZE - 1)) & !(MIN_SLOT_SIZE - 1);
        let size_index = ((size >> MIN_SLOT_SIZE_SHIFT) - 1) as usize;

        debug_assert!(size_index < MAX_IB_SIZES && size != 0, "Allocating too large index buffer slot");

        if let Some(ib_slot) = self.index_buffer_slots[size_index] {
            // found a previously allocated slot matching required size
            let next = self.index_buffer_empty_slots[ib_slot].next_same_size;
            self.index_buffer_slots[size_index] = next;
            if let Some(n) = next {
                self.index_buffer_empty_slots[n].prev_same_size = None;
            }
            Some(ib_slot)
        } else {
            // need to allocate a new slot
            self.allocate_slot_storage_ib(size)
        }
    }

    /// `W3DBufferManager::releaseSlot(W3DIndexBufferSlot *)` (W3DBufferManager.cpp:396): returns index buffer space back
    /// to pool so it can be reused later.
    pub fn release_slot_ib(&mut self, ib_slot: usize) {
        let size_index = ((self.index_buffer_empty_slots[ib_slot].size >> MIN_SLOT_SIZE_SHIFT) - 1) as usize;

        let head = self.index_buffer_slots[size_index];
        self.index_buffer_empty_slots[ib_slot].next_same_size = head;
        if let Some(h) = head {
            self.index_buffer_empty_slots[h].prev_same_size = Some(ib_slot);
        }

        self.index_buffer_slots[size_index] = Some(ib_slot);
    }

    /// `W3DBufferManager::allocateSlotStorage(Int)` (W3DBufferManager.cpp:409): reserves space inside an existing index
    /// buffer or allocates a new one to fit the required size.
    pub fn allocate_slot_storage_ib(&mut self, size: i32) -> Option<usize> {
        debug_assert!((self.num_empty_index_slots_allocated as usize) < MAX_NUMBER_SLOTS, "Nore more IB Slots");

        let mut p_ib = self.index_buffers;
        while let Some(b) = p_ib {
            if (self.empty_index_buffers[b].size - self.empty_index_buffers[b].start_free_index) >= size {
                // found enough free space in this index buffer
                if (self.num_empty_index_slots_allocated as usize) < MAX_NUMBER_SLOTS {
                    // we're allowing more slots to be allocated.
                    let s = self.num_empty_index_slots_allocated as usize;
                    let used = self.empty_index_buffers[b].used_slots;
                    let start = self.empty_index_buffers[b].start_free_index;
                    {
                        let ib_slot = &mut self.index_buffer_empty_slots[s];
                        ib_slot.size = size;
                        ib_slot.start = start;
                        ib_slot.ib = Some(b);
                        // Link to IB list of slots
                        ib_slot.next_same_ib = used;
                        ib_slot.prev_same_ib = None; // this will be the new head
                    }
                    if let Some(u) = used {
                        self.index_buffer_empty_slots[u].prev_same_ib = Some(s);
                    }
                    self.index_buffer_empty_slots[s].prev_same_size = None;
                    self.index_buffer_empty_slots[s].next_same_size = None;
                    self.empty_index_buffers[b].used_slots = Some(s);
                    self.empty_index_buffers[b].start_free_index += size;
                    self.num_empty_index_slots_allocated += 1;
                    return Some(s);
                }
            }
            p_ib = self.empty_index_buffers[b].next_ib;
        }

        let old_head = self.index_buffers; // save old list head

        // Didn't find any index buffers with room, create a new one
        debug_assert!((self.num_empty_index_buffers_allocated as usize) < MAX_INDEX_BUFFERS_CREATED, "Reached Max Static IB Shadow Geometry");

        if (self.num_empty_index_buffers_allocated as usize) < MAX_INDEX_BUFFERS_CREATED {
            let b = self.num_empty_index_buffers_allocated as usize;
            self.index_buffers = Some(b);
            self.empty_index_buffers[b].next_ib = old_head; // link to list
            self.num_empty_index_buffers_allocated += 1;

            // get new list head
            let ib_size = DEFAULT_INDEX_BUFFER_SIZE.max(size);

            // upstream: NEW_REF(DX8IndexBufferClass, (ibSize, USAGE_DEFAULT)) — device-only, not translated
            self.empty_index_buffers[b].dx8_buffer = true;
            self.empty_index_buffers[b].start_free_index = size;
            self.empty_index_buffers[b].size = ib_size;
            let s = self.num_empty_index_slots_allocated as usize;
            self.num_empty_index_slots_allocated += 1;
            self.empty_index_buffers[b].used_slots = Some(s);
            let ib_slot = &mut self.index_buffer_empty_slots[s];
            ib_slot.size = size;
            ib_slot.start = 0;
            ib_slot.ib = Some(b);
            ib_slot.prev_same_ib = None;
            ib_slot.next_same_ib = None;
            ib_slot.prev_same_size = None;
            ib_slot.next_same_size = None;
            return Some(s);
        }

        None
    }

    /// `W3DBufferManager::getNextVertexBuffer` (W3DBufferManager.h): allows iterating over vertex buffers used by the
    /// manager. Input of `None` to get first.
    pub fn get_next_vertex_buffer(&self, p_vb: Option<usize>, ty: VbmFvfTypes) -> Option<usize> {
        match p_vb {
            None => self.vertex_buffers[ty as usize],
            Some(v) => self.empty_vertex_buffers[v].next_vb,
        }
    }
}
