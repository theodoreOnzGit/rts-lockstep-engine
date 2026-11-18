//! UNTESTED DRAFT (#202, #206): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/stripoptimizer.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/stripoptimizer.h
//!
//! `StripOptimizerClass`: triangle-strip generation and ordering. A strip list is a flat `i32` sequence
//! `len, index * len, len, index * len, ...`. Upstream's raw `int*` arrays and `new`ed output become slices and `Vec`s; the
//! triangle graph (`Strip::Triangle`'s `m_neighbors`/`m_prev`/`m_next` pointers) is an index arena.
//!
//! **Findings kept as upstream has them (for the differential pass).**
//! - `getTriangleNodeConnectivityWeights` tests `weight[0] == highestVal` inside its loop over `i` (not `weight[i]`), so
//!   its result is all `+1` when vertex 0 has the highest connectivity and all `-1` otherwise.
//! - In `stripify` the search for the next triangle does not leave the neighbour loop when it finds one, so the *last*
//!   matching neighbour wins.
//! - `Combine_Strips` writes `*o++ = o[-1]`, which reads and increments `o` in one unsequenced expression (undefined in C++ before
//!   C++17); the intended value (a copy of the last index written) is what is implemented.
//! - The `Insertion_Sort` / `Quick_Sort` templates and `HashTemplateKeyClass<Edge>::Get_Hash_Value` are not used by any
//!   function in the file (the hash is a `HashMap` keyed by the edge here).

use std::collections::HashMap;

/// `Insertion_Sort(a, N)` (stripoptimizer.cpp): "class T must have operators =, > and <".
pub fn insertion_sort<T: Copy + PartialOrd>(a: &mut [T], n: usize) {
    let mut i = 1;
    while i < n {
        if a[i - 1] > a[i] {
            let v = a[i];
            let mut j = i;
            while a[j - 1] > v {
                a[j] = a[j - 1]; // copy data
                j -= 1;
                if j == 0 {
                    break;
                }
            }
            a[j] = v;
        }
        i += 1;
    }
}

/// `Quick_Sort(a, l, r)`: insertion sort for ranges of at most 17 elements, else a Hoare-style partition around `a[r]`.
pub fn quick_sort_range<T: Copy + PartialOrd>(a: &mut [T], l: i32, r: i32) {
    if r - l <= 16 {
        insertion_sort(&mut a[l as usize..], (r - l + 1) as usize);
        return;
    }
    let v = a[r as usize];
    let mut i = l - 1;
    let mut j = r;
    loop {
        loop {
            i += 1;
            if !(i < r && a[i as usize] < v) {
                break;
            }
        }
        loop {
            j -= 1;
            if !(j > 0 && a[j as usize] > v) {
                break;
            }
        }
        a.swap(i as usize, j as usize);
        if !(j > i) {
            break;
        }
    }
    let t = a[j as usize];
    a[j as usize] = a[i as usize];
    a[i as usize] = a[r as usize];
    a[r as usize] = t;
    if (i - 1) > l {
        quick_sort_range(a, l, i - 1);
    }
    if r > (i + 1) {
        quick_sort_range(a, i + 1, r);
    }
}

/// `Quick_Sort(a, N)`.
pub fn quick_sort<T: Copy + PartialOrd>(a: &mut [T], n: i32) {
    quick_sort_range(a, 0, n - 1);
}

/// `StripOptimizerClass::Get_Strip_Index_Count(strips, strip_count)`: the total of the strip lengths.
pub fn get_strip_index_count(strips: &[i32], strip_count: i32) -> i32 {
    let mut cnt = 0;
    let mut p = 0usize;
    for _ in 0..strip_count {
        let len = strips[p]; // read len
        p += 1;
        cnt += len;
        p += len as usize; // skip data
    }
    cnt
}

/// `Get_Strip_Similarity(A, B)`: the number of indices of strip `A` that occur in strip `B` (each strip starts with its
/// length).
fn get_strip_similarity(a: &[i32], b: &[i32]) -> i32 {
    let mut cnt = 0;
    let len_a = a[0] as usize;
    let len_b = b[0] as usize;
    for i in 0..len_a {
        let index = a[1 + i]; // index of A
        for j in 0..len_b {
            if b[1 + j] == index {
                // match
                cnt += 1;
                break;
            }
        }
    }
    cnt
}

/// `Copy_Strip(d, s)`: appends the strip at `s` (length first) to `d`.
fn copy_strip(d: &mut Vec<i32>, s: &[i32]) {
    let len = s[0] as usize;
    d.push(len as i32);
    for i in 0..len {
        d.push(s[1 + i]);
    }
}

/// `StripOptimizerClass::Optimize_Strip_Order(strips, strip_count)`: reorders the strips greedily so each is the one
/// sharing the most indices with the previous (ties: the first in the original order).
pub fn optimize_strip_order(strips: &mut [i32], strip_count: i32) {
    if strip_count <= 0 {
        return;
    }
    // pointers to beginning of strips
    let mut ss: Vec<Option<usize>> = Vec::with_capacity(strip_count as usize);
    let mut s = 0usize;
    for _ in 0..strip_count {
        ss.push(Some(s));
        let len = strips[s]; // read len
        s += 1;
        s += len as usize; // skip
    }
    let out_size = (get_strip_index_count(strips, strip_count) + strip_count) as usize; // output memory alloc size
    let mut out: Vec<i32> = Vec::with_capacity(out_size);
    let mut prev = ss[0].unwrap(); // previous strip
    copy_strip(&mut out, &strips[prev..]); // output first strip
    ss[0] = None;
    loop {
        let mut best_index: i32 = -1;
        let mut best_similarity: i32 = -1;
        for j in 0..strip_count as usize {
            if let Some(sj) = ss[j] {
                let sim = get_strip_similarity(&strips[prev..], &strips[sj..]); // get similarity
                if sim > best_similarity {
                    best_similarity = sim;
                    best_index = j as i32;
                }
            }
        }
        if best_index == -1 {
            // we're done
            break;
        }
        let b = ss[best_index as usize].unwrap();
        copy_strip(&mut out, &strips[b..]); // copy the strip
        prev = b; // set to prev
        ss[best_index as usize] = None; // mark as selected
    }
    // copy output
    for i in 0..out_size {
        strips[i] = out[i];
    }
}

/// `struct Tri` of `Optimize_Triangle_Order`.
#[derive(Clone, Copy)]
struct Tri {
    a: i32,
    b: i32,
    c: i32,
}

/// `Get_Similarity(A, B)`: how many of A's three vertices are among B's.
fn get_similarity(a: &Tri, b: &Tri) -> i32 {
    let mut sim = 0;
    if a.a == b.a || a.a == b.b || a.a == b.c {
        sim += 1;
    }
    if a.b == b.a || a.b == b.b || a.b == b.c {
        sim += 1;
    }
    if a.c == b.a || a.c == b.b || a.c == b.c {
        sim += 1;
    }
    sim
}

/// `StripOptimizerClass::Optimize_Triangle_Order(tris, triangle_count)`: reorders the triangles (three indices each)
/// so each follows the one sharing the most vertices with it (stopping a search early at a similarity of 2).
pub fn optimize_triangle_order(tris: &mut [i32], triangle_count: i32) {
    if triangle_count <= 0 {
        return;
    }
    let n = triangle_count as usize;
    let tri = |t: &[i32], i: usize| Tri { a: t[i * 3], b: t[i * 3 + 1], c: t[i * 3 + 2] };
    let src: Vec<Tri> = (0..n).map(|i| tri(tris, i)).collect();
    let mut t: Vec<Option<usize>> = (0..n).map(Some).collect();
    let mut out: Vec<Tri> = Vec::with_capacity(n);
    let mut prev = 0usize;
    out.push(src[prev]);
    t[0] = None;
    loop {
        // match best
        let mut best_index: i32 = -1;
        let mut best_sim: i32 = -1;
        for j in 0..n {
            if let Some(tj) = t[j] {
                let sim = get_similarity(&src[prev], &src[tj]);
                if sim > best_sim {
                    best_sim = sim;
                    best_index = j as i32;
                    if sim >= 2 {
                        // that's the best we could get
                        break;
                    }
                }
            }
        }
        if best_index == -1 {
            // we're done
            break;
        }
        let b = t[best_index as usize].unwrap();
        out.push(src[b]);
        prev = b;
        t[best_index as usize] = None;
    }
    debug_assert!(out.len() == n);
    for i in 0..n {
        tris[i * 3] = out[i].a;
        tris[i * 3 + 1] = out[i].b;
        tris[i * 3 + 2] = out[i].c;
    }
}

/// `StripOptimizerClass::Combine_Strips(strips, strip_count)`: joins the strips into one by repeating the last index of
/// each and the first of the next (and one more when the previous strip had an odd length, to keep the winding). The
/// result's first entry is the number of indices that follow.
pub fn combine_strips(strips: &[i32], strip_count: i32) -> Vec<i32> {
    let check = get_strip_index_count(strips, strip_count) + (strip_count - 1) * 3;
    let mut out: Vec<i32> = Vec::with_capacity((check + 1) as usize);
    out.push(0); // out[0]: the length, set at the end
    let mut s = 0usize;
    let mut prev_even = true;
    for i in 0..strip_count {
        let len = strips[s];
        s += 1;
        if i != 0 {
            out.push(strips[s]); // duplicate first
            if !prev_even {
                out.push(strips[s]);
            }
        }
        for _ in 0..len {
            out.push(strips[s]); // copy the strip
            s += 1;
        }
        if i != (strip_count - 1) {
            let last = *out.last().unwrap(); // *o++ = o[-1]
            out.push(last);
        }
        prev_even = len & 1 == 0;
    }
    out[0] = (out.len() - 1) as i32; // set length
    out
}

/// `Stripify` entry: `StripOptimizerClass::Stripify(tris, tri_count)`. The result is `strip_count` followed by the strips,
/// or `None` (NULL) for no triangles.
pub fn stripify(tris: &[i32], tri_count: i32) -> Option<Vec<i32>> {
    strip::stripify(tris, tri_count)
}

/// `namespace Strip`.
mod strip {
    use super::HashMap;

    /// `Strip::Edge`.
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    struct Edge {
        v: [i32; 2],
    }

    impl Edge {
        /// `sort()`: smaller vertex first.
        fn sort(&mut self) {
            if self.v[0] > self.v[1] {
                self.v.swap(0, 1);
            }
        }
    }

    /// `Strip::Triangle`; neighbours and the bin links are arena indices.
    #[derive(Clone)]
    struct Triangle {
        m_neighbors: [Option<usize>; 3],
        m_vertices: [i32; 3],
        m_prev: Option<usize>,
        m_next: Option<usize>,
        /// current bin (-1 == not in any bin)
        m_bin: i32,
    }

    impl Triangle {
        fn new() -> Self {
            Triangle { m_neighbors: [None; 3], m_vertices: [0; 3], m_prev: None, m_next: None, m_bin: -1 }
        }

        /// `getConnectivity()`: the number of neighbours.
        fn get_connectivity(&self) -> i32 {
            let mut cnt = 0;
            if self.m_neighbors[0].is_some() {
                cnt += 1;
            }
            if self.m_neighbors[1].is_some() {
                cnt += 1;
            }
            if self.m_neighbors[2].is_some() {
                cnt += 1;
            }
            cnt
        }

        /// `getEdge(i)`.
        fn get_edge(&self, i: usize) -> Edge {
            debug_assert!(i < 3);
            Edge { v: [self.m_vertices[i], if i == 2 { self.m_vertices[0] } else { self.m_vertices[i + 1] }] }
        }
    }

    /// `Strip::TriangleQueue`: triangles binned by their number of neighbours (0..=3), and per-vertex connectivity.
    struct TriangleQueue {
        m_bin: [Option<usize>; 4],
        m_node_connectivity: Vec<i32>,
    }

    impl TriangleQueue {
        /// `TriangleQueue(tris, N)`.
        fn new(tris: &mut [Triangle]) -> Self {
            let mut m_bin = [None; 4];
            let mut largest_index = 0;
            for t in tris.iter() {
                for j in 0..3 {
                    debug_assert!(t.m_vertices[j] >= 0);
                    if t.m_vertices[j] > largest_index {
                        largest_index = t.m_vertices[j];
                    }
                }
            }
            let vertex_count = (largest_index + 1) as usize;
            let mut m_node_connectivity = vec![0i32; vertex_count];
            for i in 0..tris.len() {
                let w = tris[i].get_connectivity() as usize;
                debug_assert!(tris[i].m_prev.is_none() && tris[i].m_next.is_none() && tris[i].m_bin == -1);
                tris[i].m_prev = None;
                tris[i].m_next = m_bin[w];
                if let Some(nx) = tris[i].m_next {
                    tris[nx].m_prev = Some(i);
                }
                m_bin[w] = Some(i); // head of bin
                tris[i].m_bin = w as i32; // set bin
                for j in 0..3 {
                    m_node_connectivity[tris[i].m_vertices[j] as usize] += 1;
                }
            }
            TriangleQueue { m_bin, m_node_connectivity }
        }

        /// `getTop()`: the head of the first non-empty bin.
        fn get_top(&self) -> Option<usize> {
            for i in 0..4 {
                if self.m_bin[i].is_some() {
                    return self.m_bin[i]; // return head
                }
            }
            None // end
        }

        /// `getVertexConnectivity(i)`.
        fn get_vertex_connectivity(&self, i: i32) -> i32 {
            debug_assert!(i >= 0 && (i as usize) < self.m_node_connectivity.len());
            self.m_node_connectivity[i as usize]
        }

        /// `reinsert(t)`: moves the triangle to the bin of its (new) connectivity.
        fn reinsert(&mut self, tris: &mut [Triangle], t: usize) {
            debug_assert!(tris[t].m_bin != -1); // must be in some bin
            let w = tris[t].get_connectivity() as usize;
            // remove from bin
            if let Some(p) = tris[t].m_prev {
                tris[p].m_next = tris[t].m_next;
            } else {
                debug_assert!(Some(t) == self.m_bin[tris[t].m_bin as usize]); // must be head
                self.m_bin[tris[t].m_bin as usize] = tris[t].m_next; // new head
            }
            if let Some(nx) = tris[t].m_next {
                tris[nx].m_prev = tris[t].m_prev;
            }
            tris[t].m_prev = None;
            tris[t].m_next = self.m_bin[w];
            if let Some(nx) = tris[t].m_next {
                tris[nx].m_prev = Some(t);
            }
            self.m_bin[w] = Some(t); // head of bin
            tris[t].m_bin = w as i32; // set bin
        }

        /// `removeTriangle(t)`: takes it out of its bin, cuts its links to its neighbours (which move to a lower bin) and
        /// lowers its vertices' connectivity.
        fn remove_triangle(&mut self, tris: &mut [Triangle], t: usize) {
            if let Some(p) = tris[t].m_prev {
                tris[p].m_next = tris[t].m_next;
            } else {
                debug_assert!(tris[t].m_bin != -1);
                debug_assert!(Some(t) == self.m_bin[tris[t].m_bin as usize]); // must be head
                self.m_bin[tris[t].m_bin as usize] = tris[t].m_next; // new head
            }
            if let Some(nx) = tris[t].m_next {
                tris[nx].m_prev = tris[t].m_prev;
            }
            tris[t].m_bin = -1;

            // update connectivity of t's neighbors
            let mut update: [Option<usize>; 3] = [None; 3];
            for i in 0..3 {
                update[i] = None;
                if let Some(n) = tris[t].m_neighbors[i] {
                    let mut k = 0;
                    while k < 3 {
                        if tris[n].m_neighbors[k] == Some(t) {
                            break;
                        }
                        k += 1;
                    }
                    debug_assert!(k != 3); // WASS??
                    tris[n].m_neighbors[k] = None; // reduce connection
                    tris[t].m_neighbors[i] = None;
                    update[i] = Some(n);
                }
            }

            // update connectivity count of t's vertices
            for i in 0..3 {
                let v = tris[t].m_vertices[i] as usize;
                self.m_node_connectivity[v] -= 1;
                debug_assert!(self.m_node_connectivity[v] >= 0); // WASS?
            }

            for i in 0..3 {
                // perform reinsertions now...
                if let Some(u) = update[i] {
                    self.reinsert(tris, u);
                }
            }
        }
    }

    /// `Stripify::getMod3(i)`: the lookup `s_mod[6] = {0,1,2,0,1,2}`.
    fn get_mod3(i: i32) -> usize {
        debug_assert!((0..6).contains(&i));
        [0, 1, 2, 0, 1, 2][i as usize]
    }

    /// `Stripify::getTriangleNodeConnectivityWeights(queue, tri)` (see the file header for the `weight[0]` quirk).
    fn get_triangle_node_connectivity_weights(queue: &TriangleQueue, tri: &Triangle) -> [i32; 3] {
        let mut weight = [0i32; 3];
        for i in 0..3 {
            weight[i] = queue.get_vertex_connectivity(tri.m_vertices[i]);
        }
        let mut highest_val = weight[0];
        if weight[1] > highest_val {
            highest_val = weight[1];
        }
        if weight[2] > highest_val {
            highest_val = weight[2];
        }
        let mut v = [-1, -1, -1];
        for i in 0..3 {
            if weight[0] == highest_val {
                v[i] = 1;
            }
        }
        v
    }

    /// `Stripify::generateTriangleList(inTris, N)`: copies the vertices and builds the neighbour links through an edge
    /// hash (a matching edge has the opposite direction; the edge is removed from the hash once matched).
    fn generate_triangle_list(in_tris: &[i32], n: usize) -> Vec<Triangle> {
        let mut tris: Vec<Triangle> = (0..n).map(|_| Triangle::new()).collect();
        // Copy triangle vertex data
        for i in 0..n {
            // We could perform random rotation here (this way we don't need random comparisons later in the algo in
            // equality cases).
            tris[i].m_vertices[0] = in_tris[i * 3];
            tris[i].m_vertices[1] = in_tris[i * 3 + 1];
            tris[i].m_vertices[2] = in_tris[i * 3 + 2];
        }

        // Build connectivity information using a hash table
        let mut hash: HashMap<Edge, usize> = HashMap::new();
        for i in 0..n {
            for j in 0..3 {
                if tris[i].m_neighbors[j].is_none() {
                    // if neighbor not defined yet
                    let edge = tris[i].get_edge(j);
                    let mut e = edge;
                    e.sort(); // sort vertices (smaller first)
                    if let Some(&nb) = hash.get(&e) {
                        // if edge is already in the hash...
                        let mut k = 0;
                        while k < 3 {
                            // find matching edge
                            if tris[nb].m_neighbors[k].is_none() {
                                // this neighbor not located yet
                                let ek = tris[nb].get_edge(k); // get edge
                                if ek.v[0] == edge.v[1] && ek.v[1] == edge.v[0] {
                                    // if matching edge (note that order must be different)
                                    break; // .. we found the edge
                                }
                            }
                            k += 1;
                        }
                        if k < 3 {
                            // (k==3) -> no match
                            tris[i].m_neighbors[j] = Some(nb); // set neighbor
                            tris[nb].m_neighbors[k] = Some(i); // set neighbor
                            hash.remove(&e); // remove from hash (this speeds up the hash queries considerably)
                        }
                    } else {
                        hash.insert(e, i); // insert triangle into hash
                    }
                }
            }
        }
        tris
    }

    /// `Stripify::stripify(inTris, N)`: repeatedly takes the triangle with the fewest neighbours and walks to a neighbour,
    /// inserting swap vertices where the strip would otherwise turn the wrong way. Returns `strip_count` then the strips.
    pub fn stripify(in_tris: &[i32], n: i32) -> Option<Vec<i32>> {
        if in_tris.is_empty() || n <= 0 {
            // boo!
            return None;
        }
        let n = n as usize;

        // Initial setup
        let mut tris = generate_triangle_list(in_tris, n); // build connectivity info
        let mut out: Vec<i32> = Vec::with_capacity(n * 5); // absolute worst case situation
        let mut strip_count = 0; // number of output strips
        let mut queue = TriangleQueue::new(&mut tris); // insert triangles into priority queue
        let mut _n_swaps = 0;
        let mut _n_len = 0;

        // Main loop. Always select triangle with smallest weight.
        loop {
            let mut t = match queue.get_top() {
                // get triangle with smallest weight
                Some(t) => t,
                None => break, // ok, we ran out of triangles (we're done)
            };

            // Choose initial direction by selecting neighbor with smallest weight
            let p_len = out.len(); // current position (as we need to take care of it later)
            let mut len: i32 = 3; // initial length
            let mut best: i32 = 0; // best edge
            let mut best_weight: i32 = 0x7fff_ffff; // initialize to maximum width
            let node_weights = get_triangle_node_connectivity_weights(&queue, &tris[t]);
            for i in 0..3 {
                if let Some(nb) = tris[t].m_neighbors[i] {
                    // if triangle has a neighbor in this direction
                    let mut weight = tris[nb].get_connectivity(); // get connectivity of neighbor
                    weight += node_weights[i]; // add node weights
                    weight += node_weights[get_mod3(i as i32 + 1)];
                    // DEBUG DEBUG ADD SWAP COST HERE?
                    if weight <= best_weight {
                        best_weight = weight;
                        best = i as i32;
                    }
                }
            }
            out.push(len); // output the first three vertices
            out.push(tris[t].m_vertices[get_mod3(best + 2)]);
            out.push(tris[t].m_vertices[get_mod3(best)]);
            out.push(tris[t].m_vertices[get_mod3(best + 1)]);

            loop {
                let mut next: Option<usize> = None; // find next triangle
                for i in 0..3 {
                    if let Some(nb) = tris[t].m_neighbors[i] {
                        for j in 0..3 {
                            let mut e = tris[nb].get_edge(j);
                            if len & 1 == 0 {
                                e.v.swap(0, 1); // swap
                            }
                            let o1 = out[out.len() - 1];
                            let o2 = out[out.len() - 2];
                            if e.v[0] == o1 && e.v[1] == o2 {
                                next = Some(nb);
                                break;
                            }
                        }
                    }
                }
                queue.remove_triangle(&mut tris, t); // get rid of the old triangle
                let next = match next {
                    Some(nx) => nx,
                    None => break, // we're done
                };

                // Find out where we want to continue...
                let mut best_edge: i32 = -1;
                let mut best_weight: i32 = 0x7fff_ffff;
                let mut best_swap = false;
                let node_weights = get_triangle_node_connectivity_weights(&queue, &tris[next]);
                for i in 0..3 {
                    if let Some(nb) = tris[next].m_neighbors[i] {
                        // is there a neighbor?
                        let e = tris[next].get_edge(i); // a swap happens if it contains the prevprev
                        let o2 = out[out.len() - 2];
                        let swap = e.v[0] == o2 || e.v[1] == o2;
                        let mut w = tris[nb].get_connectivity();
                        w += node_weights[i]; // add vertex weight
                        w += node_weights[get_mod3(i as i32 + 1)]; // add vertex weight
                        w += if swap { 1 } else { -1 }; // add swap penalty
                        if w <= best_weight {
                            best_weight = w;
                            best_edge = i as i32;
                            best_swap = swap;
                        }
                    }
                }
                if best_edge != -1 && best_swap {
                    // if we're going to continue...
                    let v = out[out.len() - 2];
                    out.push(v); // introduce swap vertex
                    len += 1; // adjust length
                    _n_swaps += 1; // update statistics
                }

                // Find out which was the new vertex (store it to vIndex)
                let o1 = out[out.len() - 1];
                let o2 = out[out.len() - 2];
                let nv = tris[next].m_vertices;
                let mut v_index = 0;
                if nv[1] != o1 && nv[1] != o2 {
                    v_index = 1;
                } else if nv[2] != o1 && nv[2] != o2 {
                    v_index = 2;
                } else {
                    debug_assert!(nv[0] != o1 && nv[0] != o2);
                }

                // Output the vertex and move to next triangle
                out.push(nv[v_index]); // output the vertex
                len += 1; // increase strip length
                t = next; // move to next triangle
            }
            out[p_len] = len; // patch final length
            strip_count += 1; // increase strip count
            _n_len += len;
        }

        // Allocate new optimal-size array and copy output there.
        let mut r_out: Vec<i32> = Vec::with_capacity(out.len() + 1);
        r_out.push(strip_count); // first entry is number of strips
        for &v in out.iter() {
            debug_assert!(v >= 0); // would be nice to test for len as well
            r_out.push(v);
        }
        Some(r_out)
    }
}
