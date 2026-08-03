//! Sender-provided visual materialization

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use std::os::unix::ffi::OsStrExt;

use rustix::fs::{openat2, Mode, OFlags, ResolveFlags, CWD};
use unixnotis_core::{
    decode_image_asset_contents, AssetPolicy, ImageData, NotificationAttribution,
    DEFAULT_ICON_ASSET_EXTENSIONS, DEFAULT_ICON_ASSET_MAX_HEIGHT, DEFAULT_ICON_ASSET_MAX_PIXELS,
    DEFAULT_ICON_ASSET_MAX_WIDTH,
};
use url::Url;
use zbus::zvariant::OwnedValue;

use crate::daemon::notifications::identity::DesktopIdentityIndex;

use super::owned_to_string;

pub(in crate::daemon::notifications::ingress) const MAX_SENDER_VISUAL_BYTES: u64 = 2_097_152;
const MAX_STORED_AVATAR_DIMENSION: u32 = 64;
pub(in crate::daemon::notifications::ingress) const MAX_DECODE_DIMENSION: u32 =
    MAX_STORED_AVATAR_DIMENSION * 8;
pub(in crate::daemon::notifications) const MAX_STORED_CONTENT_DIMENSION: u32 = 256;

pub(in crate::daemon::notifications) const CONVERSATION_AVATAR_TIMEOUT: Duration =
    Duration::from_millis(500);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::daemon::notifications) enum SenderVisualRole {
    None,
    ConversationAvatar,
    ApplicationProvidedIcon,
}

pub(in crate::daemon::notifications) const fn may_materialize_application_icon(
    attribution: &NotificationAttribution,
) -> bool {
    attribution.may_materialize_application_icon()
}

pub(in crate::daemon::notifications) const fn may_materialize_content_image(
    attribution: &NotificationAttribution,
) -> bool {
    attribution.may_materialize_content_image()
}

pub(in crate::daemon::notifications) fn sender_visual_role(
    attribution: &NotificationAttribution,
    index: &DesktopIdentityIndex,
    hints: &HashMap<String, OwnedValue>,
    actions: &[String],
    app_icon: &str,
) -> SenderVisualRole {
    // Sender paths are never opened until attribution grants positive local evidence
    if !may_materialize_application_icon(attribution) {
        return SenderVisualRole::None;
    }

    // An advertised inline reply is an explicit communication signal
    if actions
        .chunks_exact(2)
        .any(|pair| pair.first().is_some_and(|key| key == "inline-reply"))
    {
        return SenderVisualRole::ConversationAvatar;
    }

    // Category hints remain presentation input, not identity proof
    let explicit_metadata =
        hints
            .get("category")
            .and_then(owned_to_string)
            .is_some_and(|category| {
                let category = category.to_ascii_lowercase();
                ["im", "chat", "message", "email", "mail"]
                    .iter()
                    .any(|marker| category.split('.').any(|part| part == *marker))
            });
    // Desktop categories cover clients that omit the optional wire category
    let desktop_metadata = index.desktop_id_has_communication_role(&attribution.desktop_id);
    if explicit_metadata || desktop_metadata {
        SenderVisualRole::ConversationAvatar
    } else if local_avatar_path(app_icon).is_some() {
        SenderVisualRole::ApplicationProvidedIcon
    } else {
        SenderVisualRole::None
    }
}

pub(in crate::daemon::notifications) fn materialize_sender_visual(
    app_icon: &str,
    max_dimension: u32,
) -> Option<ImageData> {
    // Convert the sender value to a local path before touching the filesystem
    let path = local_avatar_path(app_icon)?;
    let descriptor = openat2(
        CWD,
        &path,
        OFlags::RDONLY
            .union(OFlags::NONBLOCK)
            .union(OFlags::CLOEXEC)
            .union(OFlags::NOFOLLOW),
        Mode::empty(),
        ResolveFlags::NO_MAGICLINKS,
    )
    .ok()?;
    let mut file = std::fs::File::from(descriptor);
    // Metadata is taken from the opened descriptor, not from a second path lookup
    let metadata = file.metadata().ok()?;
    if !sender_visual_file_allowed(metadata.is_file(), metadata.len()) {
        return None;
    }

    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_SENDER_VISUAL_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if !avatar_buffer_size_allowed(bytes.len()) {
        return None;
    }

    // Keep the decoder bound independent from the UI-requested size
    let max_dimension = bounded_decode_dimension(max_dimension);
    let policy = AssetPolicy {
        max_bytes: MAX_SENDER_VISUAL_BYTES,
        max_width: DEFAULT_ICON_ASSET_MAX_WIDTH.min(max_dimension),
        max_height: DEFAULT_ICON_ASSET_MAX_HEIGHT.min(max_dimension),
        max_pixels: DEFAULT_ICON_ASSET_MAX_PIXELS
            .min(u64::from(max_dimension).checked_mul(u64::from(max_dimension))?),
        allowed_extensions: DEFAULT_ICON_ASSET_EXTENSIONS,
    };
    let decoded = decode_image_asset_contents(&path, &bytes, policy).ok()?;
    let (width, height, rgba) =
        downsample_avatar(decoded.width, decoded.height, decoded.rgba, max_dimension)?;
    let width = i32::try_from(width).ok()?;
    let height = i32::try_from(height).ok()?;
    let rowstride = width.checked_mul(4)?;
    let expected = usize::try_from(rowstride)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?;
    (rgba.len() == expected).then_some(ImageData {
        width,
        height,
        rowstride,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: rgba,
    })
}

pub(in crate::daemon::notifications::ingress) fn downsample_avatar(
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    max_dimension: u32,
) -> Option<(u32, u32, Vec<u8>)> {
    if width == 0 || height == 0 {
        return None;
    }
    let source_pixels = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?;
    if rgba.len() != source_pixels.checked_mul(4)? {
        return None;
    }

    let (target_width, target_height) = if width >= height {
        (
            max_dimension.min(width),
            width_to_height(width, height, max_dimension),
        )
    } else {
        (
            height_to_width(width, height, max_dimension),
            max_dimension.min(height),
        )
    };
    let target_pixels = usize::try_from(target_width)
        .ok()?
        .checked_mul(usize::try_from(target_height).ok()?)?;
    let max_pixels = usize::try_from(max_dimension)
        .ok()?
        .checked_mul(usize::try_from(max_dimension).ok()?)?;
    if target_pixels > max_pixels {
        return None;
    }
    if target_width == width && target_height == height {
        return Some((width, height, rgba));
    }

    let mut output = vec![0u8; target_pixels.checked_mul(4)?];
    for target_y in 0..target_height {
        let source_y = usize::try_from(target_y)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?
            / usize::try_from(target_height).ok()?;
        for target_x in 0..target_width {
            let source_x = usize::try_from(target_x)
                .ok()?
                .checked_mul(usize::try_from(width).ok()?)?
                / usize::try_from(target_width).ok()?;
            let source_index = source_y
                .checked_mul(usize::try_from(width).ok()?)?
                .checked_add(source_x)?
                .checked_mul(4)?;
            let target_index = usize::try_from(target_y)
                .ok()?
                .checked_mul(usize::try_from(target_width).ok()?)?
                .checked_add(usize::try_from(target_x).ok()?)?
                .checked_mul(4)?;
            output[target_index..target_index + 4]
                .copy_from_slice(&rgba[source_index..source_index + 4]);
        }
    }
    Some((target_width, target_height, output))
}

fn width_to_height(width: u32, height: u32, target_width: u32) -> u32 {
    if width <= target_width {
        return height;
    }
    u64::from(height)
        .saturating_mul(u64::from(target_width))
        .checked_div(u64::from(width))
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1)
        .max(1)
}

fn height_to_width(width: u32, height: u32, target_height: u32) -> u32 {
    if height <= target_height {
        return width;
    }
    u64::from(width)
        .saturating_mul(u64::from(target_height))
        .checked_div(u64::from(height))
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1)
        .max(1)
}

pub(in crate::daemon::notifications::ingress) const fn avatar_file_size_allowed(size: u64) -> bool {
    size <= MAX_SENDER_VISUAL_BYTES
}

pub(in crate::daemon::notifications::ingress) const fn avatar_buffer_size_allowed(
    size: usize,
) -> bool {
    size <= MAX_SENDER_VISUAL_BYTES as usize
}

pub(in crate::daemon::notifications::ingress) const fn sender_visual_file_allowed(
    is_regular: bool,
    size: u64,
) -> bool {
    is_regular && avatar_file_size_allowed(size)
}

pub(in crate::daemon::notifications::ingress) fn bounded_decode_dimension(requested: u32) -> u32 {
    std::cmp::min(requested, MAX_DECODE_DIMENSION)
}

pub(in crate::daemon::notifications::ingress) fn local_avatar_path(value: &str) -> Option<PathBuf> {
    if value.starts_with('/') {
        return Some(PathBuf::from(value));
    }
    if !valid_percent_escapes(value) {
        return None;
    }
    let url = Url::parse(value).ok()?;
    if url.scheme() != "file" || url.query().is_some() || url.fragment().is_some() {
        return None;
    }
    match url.host_str() {
        None | Some("" | "localhost") => {}
        Some(_) => return None,
    }
    let path = url.to_file_path().ok()?;
    (!path.as_os_str().as_bytes().contains(&0)).then_some(path)
}

pub(in crate::daemon::notifications::ingress) fn valid_percent_escapes(value: &str) -> bool {
    // Url accepts some malformed percent text literally, so reject it before parsing
    let mut bytes = value.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte != b'%' {
            continue;
        }
        let Some(first) = bytes.next() else {
            return false;
        };
        let Some(second) = bytes.next() else {
            return false;
        };
        if !first.is_ascii_hexdigit() || !second.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}
