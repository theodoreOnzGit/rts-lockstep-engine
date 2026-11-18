//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/htree.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/htree.h
//!
//! The tree-building statics of `HTreeClass` no earlier pass translated: `Alter_Avatar_HTree`, `Create_Morphed` and the
//! two `Create_Interpolated`. Each clones a tree (`new HTreeClass(*tree)` is `Clone`) and rewrites the `BaseTransform`
//! translations. They work on the verified [`HTree`] / [`Pivot`] (formats::w3d::htree). The `assert`s are debug
//! assertions and are kept as `debug_assert!`.

use crate::formats::w3d::htree::HTree;
use crate::ww::vector3::Vector3;
use crate::ww::wwmath;

/// The C string in a `char Name[W3D_NAME_LEN]` (up to the first NUL).
fn c_name(name: &[u8]) -> &[u8] {
    let n = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    &name[..n]
}

/// `HTreeClass::Alter_Avatar_HTree(tree, scale)` (htree.cpp:1072): returns the clone of `tree` with every pivot that has
/// a parent re-positioned so the avatar skeleton is scaled by `scale` per axis in world space. Pivots on `flip_list` (the
/// arms and hands, stretched along Y in the T-pose) use the Z scale for Y.
///
/// A pivot whose `Parent` index is one past the end (`Load_W3D` can form that) reads off the array upstream; here such a
/// pivot is skipped like a parentless one.
pub fn alter_avatar_htree(tree: &HTree, scale: &Vector3) -> HTree {
    // This is a specific list of pivot names used in the avatar meshes that we need to special case for scaling.
    let flip_list: [&[u8]; 10] = [
        b" RIGHTFOREARM",
        b" RIGHTHAND",
        b" LEFTFOREARM",
        b" LEFTHAND",
        b"RIGHTINDEX",
        b"RIGHTFINGERS",
        b"RIGHTTHUMB",
        b"LEFTINDEX",
        b"LEFTFINGERS",
        b"LEFTTHUMB",
    ];

    // Clone the new tree with the tree that is passed in
    let mut new_tree = tree.clone();

    // Go through each of the pivots and calculate and transform the pivots to match the desired scaling factor
    for pi in 0..new_tree.num_pivots as usize {
        let piv = tree.pivots[pi];
        let mut adjusted_scale = *scale;

        // If there is no parent, skip
        let Some(parent_index) = piv.parent else { continue };
        let Some(parent) = tree.pivots.get(parent_index as usize) else { continue };

        // If the pivot is on the flip list, use the Z scale to scale both the Z & Y axis
        let mut index = 0;
        loop {
            if index >= flip_list.len() {
                break;
            } else if c_name(&piv.name) == flip_list[index] {
                adjusted_scale.y = scale.z;
                break;
            }
            index += 1;
        }

        // Get the positions of the pivot and the pivot's parent in worldspace & apply the altering scale to it
        let mut pivot_pos = piv.transform.get_translation();
        let mut pivot_parent_pos = parent.transform.get_translation();
        pivot_pos.scale(&adjusted_scale);
        pivot_parent_pos.scale(&adjusted_scale);

        // Get the pivot's parent's inverse transform
        let parent_inverse_transform = parent.transform.get_inverse();

        // Get the new desired vector in worldspace
        let mut new_relative_vector = pivot_pos - pivot_parent_pos;

        // Rotate the new vector by the pivot's parent's inverse transform to put it in local space
        new_relative_vector = parent_inverse_transform.rotate_vector(&new_relative_vector);

        // Store the final result in the new HTree
        new_tree.pivots[pi].base_transform.set_translation(&new_relative_vector);
    }

    new_tree
}

/// `HTreeClass::Create_Morphed(num_morph_sources, morph_weights, tree_array)` (htree.cpp:1129): "Morph the bones on the
/// HTree using weights from a number of other HTrees": the first tree cloned, each pivot's translation the weighted sum
/// of the sources' `BaseTransform` translations.
pub fn create_morphed(num_morph_sources: i32, morph_weights: &[f32], tree_array: &[&HTree]) -> HTree {
    debug_assert!(num_morph_sources > 0);
    for i in 0..num_morph_sources as usize {
        debug_assert!(morph_weights[i] >= 0.0 && morph_weights[i] <= 1.0);
    }
    for i in 0..(num_morph_sources - 1) as usize {
        debug_assert!(tree_array[i].num_pivots == tree_array[i + 1].num_pivots);
    }

    // Clone the first one,
    let mut new_tree = tree_array[0].clone();

    // Then interpolate all the pivots translations
    for pi in 0..new_tree.num_pivots as usize {
        let mut pos = Vector3::new(0.0, 0.0, 0.0);
        for nm in 0..num_morph_sources as usize {
            pos.x += tree_array[nm].pivots[pi].base_transform.get_translation().x * morph_weights[nm];
            pos.y += tree_array[nm].pivots[pi].base_transform.get_translation().y * morph_weights[nm];
            pos.z += tree_array[nm].pivots[pi].base_transform.get_translation().z * morph_weights[nm];
        }

        new_tree.pivots[pi].base_transform.set_translation(&pos);
    }

    new_tree
}

/// `HTreeClass::Create_Interpolated(tree_a0_b0, tree_a0_b1, tree_a1_b0, tree_a1_b1, lerp_a, lerp_b)` (htree.cpp:1163):
/// "Create an HTree by Interpolating between others": bilinear interpolation of the translations, `lerp_b` first (within
/// each `a`), then `lerp_a`.
pub fn create_interpolated_bilinear(
    tree_a0_b0: &HTree,
    tree_a0_b1: &HTree,
    tree_a1_b0: &HTree,
    tree_a1_b1: &HTree,
    lerp_a: f32,
    lerp_b: f32,
) -> HTree {
    debug_assert!(tree_a0_b0.num_pivots == tree_a0_b1.num_pivots);
    debug_assert!(tree_a0_b0.num_pivots == tree_a1_b0.num_pivots);
    debug_assert!(tree_a0_b0.num_pivots == tree_a1_b1.num_pivots);

    // Clone the first one,
    let mut new_tree = tree_a0_b0.clone();

    // Then interpolate all the pivots translations
    for pi in 0..new_tree.num_pivots as usize {
        let pos_a0 = Vector3::lerp(
            &tree_a0_b0.pivots[pi].base_transform.get_translation(),
            &tree_a0_b1.pivots[pi].base_transform.get_translation(),
            lerp_b,
        );
        let pos_a1 = Vector3::lerp(
            &tree_a1_b0.pivots[pi].base_transform.get_translation(),
            &tree_a1_b1.pivots[pi].base_transform.get_translation(),
            lerp_b,
        );
        let pos = Vector3::lerp(&pos_a0, &pos_a1, lerp_a);

        new_tree.pivots[pi].base_transform.set_translation(&pos);
    }

    new_tree
}

/// `HTreeClass::Create_Interpolated(tree_base, tree_a, tree_b, a_scale, b_scale)` (htree.cpp:1195): the clone of
/// `tree_base`, and only if `|a_scale| + |b_scale| > 0` each translation becomes the `|scale|`-weighted mean of
/// `Lerp(base, a, a_scale)` and `Lerp(base, b, b_scale)` (so a negative scale extrapolates away from the target).
pub fn create_interpolated(tree_base: &HTree, tree_a: &HTree, tree_b: &HTree, a_scale: f32, b_scale: f32) -> HTree {
    debug_assert!(tree_base.num_pivots == tree_a.num_pivots);
    debug_assert!(tree_base.num_pivots == tree_b.num_pivots);

    // Clone the first one,
    let mut new_tree = tree_base.clone();

    let a_scale_abs = wwmath::fabs(a_scale);
    let b_scale_abs = wwmath::fabs(b_scale);

    if a_scale_abs + b_scale_abs > 0.0 {
        // Then interpolate all the pivots translations
        for pi in 0..new_tree.num_pivots as usize {
            let pos_a = Vector3::lerp(
                &tree_base.pivots[pi].base_transform.get_translation(),
                &tree_a.pivots[pi].base_transform.get_translation(),
                a_scale,
            );
            let pos_b = Vector3::lerp(
                &tree_base.pivots[pi].base_transform.get_translation(),
                &tree_b.pivots[pi].base_transform.get_translation(),
                b_scale,
            );

            let pos = (pos_a * a_scale_abs + pos_b * b_scale_abs) / (a_scale_abs + b_scale_abs);

            new_tree.pivots[pi].base_transform.set_translation(&pos);
        }
    }

    new_tree
}
