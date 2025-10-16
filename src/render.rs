//! Render a docker image.

// Docker image format is specified at
// https://github.com/moby/moby/blob/v17.05.0-ce/image/spec/v1.md

use std::io::{ErrorKind, Read};
use std::{fs, io, path};
use std::path::Path;
use libflate::gzip;
use tar::EntryType;

#[derive(Debug)]
pub struct LayerBlob {
  pub bytes: Vec<u8>,
  pub media_type: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
  #[error("wrong target path {}: must be absolute path to existing directory", _0.display())]
  WrongTargetPath(path::PathBuf),
  #[error("io error")]
  Io(#[from] std::io::Error),
}

/// Unpack an ordered list of layers to a target directory.
///
/// Layers must be provided as gzip-compressed tar archives, with lower layers
/// coming first. Target directory must be an existing absolute path.
pub fn unpack(layers: &[Vec<u8>], target_dir: &Path) -> Result<(), RenderError> {
  filter_unpack(layers, target_dir, |_| { true })
}

/// Unpack an ordered list of layers-blobs to a target directory.
///
/// Layers must be provided as gzip- or zstd-compressed tar archives, with lower layers
/// coming first. Target directory must be an existing absolute path.
pub fn unpack_layers(layers: &[LayerBlob], target_dir: &Path) -> Result<(), RenderError> {
  filter_unpack_layers(layers, target_dir,|_| { true } )
}

pub fn filter_unpack<P>(layers: &[Vec<u8>], target_dir: &Path, predicate: P) -> Result<(), RenderError>
where
    P: Fn(&Path) -> bool,
{
  let layers = layers
      .into_iter()
      .map(|b| LayerBlob { bytes: b.clone(), media_type: None })
      .collect::<Vec<_>>();

  filter_unpack_layers(layers.as_slice(), target_dir, predicate)
}

/// Unpack an ordered list of layers to a target directory, filtering
/// file entries by path.
///
/// Layers must be provided as gzip- or zstd-compressed tar archives, with lower layers
/// coming first. Target directory must be an existing absolute path.
pub fn filter_unpack_layers<P>(layers: &[LayerBlob], target_dir: &Path, predicate: P) -> Result<(), RenderError>
where
  P: Fn(&Path) -> bool,
{
  for l in layers {
    _unpack_layer(l, target_dir, &predicate)?;
  }
  Ok(())
}

fn _unpack_archive<'a, P>(dst: &Path, archive: &mut tar::Archive<Box<dyn Read + 'a>>, predicate: P) -> io::Result<()>
where P: Fn(&Path) -> bool
{
  if dst.symlink_metadata().is_err() {
    fs::create_dir_all(&dst)
        .map_err(|e| io::Error::new( ErrorKind::Other, format!("failed to create `{}`. {}", dst.display(), e)))?
  };

  // Canonicalizing the dst directory will prepend the path with '\\?\'
  // on windows which will allow windows APIs to treat the path as an
  // extended-length path with a 32,767 character limit. Otherwise all
  // unpacked paths over 260 characters will fail on creation with a
  // NotFound exception.
  let dst = &dst.canonicalize().unwrap_or(dst.to_path_buf());

  // Delay any directory entries until the end (they will be created if needed by
  // descendants), to ensure that directory permissions do not interfer with descendant
  // extraction.
  let mut directories = Vec::new();
  for entry in archive.entries()? {
    let mut file = entry.map_err(|e| io::Error::new(ErrorKind::Other, format!("failed to iterate over archive. {}", e)))?;
    if file.header().entry_type() == EntryType::Directory {
      directories.push(file);
    } else {
      // Check for whiteouts else unpack file
      let path = file.path()?;
      let parent = path.parent().unwrap_or_else(|| Path::new("/"));

      if let Some(fname) = path.file_name() {
        let wh_name = fname.to_string_lossy();
        if wh_name == ".wh..wh..opq" {
          //TODO(lucab): opaque whiteout, dir removal
        } else if wh_name.starts_with(".wh.") {
          let rel_parent = path::PathBuf::from("./".to_string() + &parent.to_string_lossy());

          // Remove real file behind whiteout
          let real_name = wh_name.trim_start_matches(".wh.");
          let abs_real_path = dst.join(&rel_parent).join(real_name);
          remove_whiteout(abs_real_path)?;

          // Remove whiteout place-holder
          let abs_wh_path = dst.join(&rel_parent).join(fname);
          remove_whiteout(abs_wh_path)?;
        } else {
          if predicate(&path) {
            //println!("unpack {}", file.path()?.display());
            file.unpack_in(dst)?;
          }
        }
      }
    }
  }

  // Apply the directories.
  //
  // Note: the order of application is important to permissions. That is, we must traverse
  // the filesystem graph in topological ordering or else we risk not being able to create
  // child directories within those of more restrictive permissions. See [0] for details.
  //
  // [0]: <https://github.com/alexcrichton/tar-rs/issues/242>
  directories.sort_by(|a, b| b.path_bytes().cmp(&a.path_bytes()));
  for mut dir in directories {
    dir.unpack_in(dst)?;
  }

  Ok(())
}

fn _unpack_layer<'a, P>(layer: &'a LayerBlob, target_dir: &Path, predicate: &P) -> Result<(), RenderError>
where P: Fn(&Path) -> bool
{
  if !target_dir.is_absolute() || !target_dir.exists() || !target_dir.is_dir() {
    return Err(RenderError::WrongTargetPath(target_dir.to_path_buf()));
  }

  let decompressed_reader: Box<dyn Read + 'a> = {
    let l = &layer.bytes;

    match layer.media_type {
      Some(ref media_type) if media_type.ends_with("+zstd") => {
        Box::new(zstd::Decoder::new(l.as_slice())?)
      }
      _ => {
        Box::new(gzip::Decoder::new(l.as_slice())?)
      }
    }
  };

  // Unpack layers
  let mut archive = tar::Archive::new(decompressed_reader);
  archive.set_preserve_permissions(true);
  archive.set_unpack_xattrs(true);

  _unpack_archive(target_dir, &mut archive, predicate)?;

  Ok(())
}

// Whiteout files in archive may not exist on filesystem if they were
// filtered out via filter_unpack.  If not found, that's ok and the
// error is non-fatal.  Otherwise still return error for other
// failures.
fn remove_whiteout(path: path::PathBuf) -> io::Result<()> {
  let res = fs::remove_dir_all(path);

  match res {
    Ok(_) => res,
    Err(ref e) => match e.kind() {
      io::ErrorKind::NotFound => Ok(()),
      _ => res,
    },
  }
}
