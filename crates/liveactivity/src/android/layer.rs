//! A layout activity's push on the device (liveupdate 0.12.0): liveupdate's push entry hands the member
//! `liveupdate.layer` of a data message to this crate's, which runs it as the application would, with the device's
//! projection and the backend's store, and answers liveupdate's outcome for the AAR to store as a `pushed` event.

use super::{AndroidFiles, LiveUpdateBackend};
use crate::{
    Error, LayoutEndInput, LayoutUpdateInput, Result, Validate,
    push::layer::{Layer, Wire},
};
use liveupdate::{Driver, ErrorCode, push::outcome};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where the backend remembers its files, under the folder of the app's files, for a push in a process that a
/// message started.
const REMEMBERED: &str = "liveactivity/push.json";

impl<D: Driver + 'static> LiveUpdateBackend<D> {
    /// Runs the member `liveupdate.layer` of a data message as the application would run its request, and answers
    /// liveupdate's outcome (`liveupdate::push::outcome`): the command, the key, the revision, and the activity's
    /// id or the code of a refusal. An update or an end acts on the live activity of the key. A member that is no
    /// message of this version names no command, and the AAR stores nothing of it. `LiveUpdatePush.handle` calls
    /// this through the native entry of the feature `android-push`; a host that receives a message in Rust calls it
    /// itself.
    pub fn push_layer(&self, member: &str) -> String {
        let Some(wire) = Wire::read(member) else {
            return "{}".to_owned();
        };
        let (command, key, revision) = (
            wire.command.clone(),
            wire.activity_key.clone(),
            wire.revision(),
        );
        // An outcome that the AAR's event store refuses would be lost the same way, unexplained.
        if !storable(&key, revision) {
            return "{}".to_owned();
        }
        match self.run_layer(wire) {
            Ok(id) => outcome(&command, &key, revision, Ok(&id)),
            Err(error) => outcome(&command, &key, revision, Err(code(&error))),
        }
    }

    fn run_layer(&self, wire: Wire) -> Result<String> {
        let Some((layouts, _)) = &self.layouts else {
            return Err(Error::unsupported_feature(
                "Layout activities need the app's files",
            ));
        };
        let key = wire.activity_key.clone();
        let live = || {
            layouts
                .live_id(&self.engine, &key)?
                .ok_or_else(|| Error::new("NOT_FOUND", "No live activity has this key"))
        };
        match wire.layer()? {
            Layer::Start(input) => {
                input.validate()?;
                // FCM delivers a message at least once: the same start after its end answers the ended activity.
                if let Some(id) = layouts.ended_start(&self.engine, &input)? {
                    return Ok(id);
                }
                Ok(layouts.start(&self.engine, *input)?.id)
            }
            // The request is checked before its activity is looked for, as the application's call is.
            Layer::Update { snapshot, alert } => {
                let input = LayoutUpdateInput {
                    alert,
                    ..LayoutUpdateInput::new(key.clone(), snapshot)
                };
                input.validate()?;
                let input = LayoutUpdateInput {
                    id: live()?,
                    ..input
                };
                Ok(layouts.update(&self.engine, input)?.id)
            }
            Layer::End { snapshot, policy } => {
                let input = LayoutEndInput {
                    id: key.clone(),
                    policy,
                    snapshot,
                };
                input.validate()?;
                let input = LayoutEndInput {
                    id: live()?,
                    ..input
                };
                Ok(layouts.end(&self.engine, input)?.id)
            }
        }
    }
}

/// The largest revision the AAR's event store takes, as JSON's safe integers.
const MAX_REVISION: u64 = 9_007_199_254_740_991;
/// The longest key the AAR's event store takes.
const MAX_KEY: usize = 128;

/// Whether the AAR's event store takes an outcome of `key` and `revision`: a key of a letter or a digit and then
/// letters, digits, `.`, `_`, `:` and `-`, at most 128, and a revision of JSON's safe integers.
fn storable(key: &str, revision: Option<u64>) -> bool {
    let mut bytes = key.bytes();
    let first = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric());
    first
        && key.len() <= MAX_KEY
        && bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
        && revision.is_none_or(|revision| revision <= MAX_REVISION)
}

/// liveupdate's code of a refusal: the contract's, where liveupdate has it.
fn code(error: &Error) -> ErrorCode {
    match error.code.as_str() {
        "PAYLOAD_TOO_LARGE" => ErrorCode::InvalidInput,
        code => {
            serde_json::from_value(serde_json::Value::from(code)).unwrap_or(ErrorCode::NativeError)
        }
    }
}

/// What a backend of the application keeps of its files, for a push that builds one in another process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Remembered {
    app_name: String,
    layout_store: PathBuf,
    image_folder: String,
    samsung_now_bar: bool,
}

/// Writes what `files` says beside them. A file that cannot be written leaves a push to answer `NATIVE_ERROR`.
#[cfg_attr(
    not(feature = "android-push"),
    allow(
        dead_code,
        reason = "a backend remembers its files with the feature `android-push`"
    )
)]
pub(crate) fn remember(files: &AndroidFiles) -> std::io::Result<()> {
    let path = files.files.join(REMEMBERED);
    let folder = path.parent().unwrap_or(&files.files);
    std::fs::create_dir_all(folder)?;
    let remembered = Remembered {
        app_name: files.app_name.clone(),
        layout_store: files.layout_store.clone(),
        image_folder: files.image_folder.clone(),
        samsung_now_bar: files.samsung_now_bar,
    };
    let text = serde_json::to_vec(&remembered).map_err(std::io::Error::other)?;
    // Written only when it changes, and whole: a push that reads it meanwhile reads the old or the new.
    if std::fs::read(&path).is_ok_and(|kept| kept == text) {
        return Ok(());
    }
    let written = folder.join(format!(".push.json.{}", std::process::id()));
    std::fs::write(&written, text)?;
    std::fs::rename(written, path)
}

/// The files that a backend of the application last had under `folder`, the app's `Context.getFilesDir()`.
#[cfg_attr(
    not(all(feature = "android-push", target_os = "android")),
    allow(
        dead_code,
        reason = "the native entry is Android's, with the feature `android-push`"
    )
)]
pub(crate) fn remembered(folder: &Path) -> Option<AndroidFiles> {
    let text = std::fs::read(folder.join(REMEMBERED)).ok()?;
    let remembered: Remembered = serde_json::from_slice(&text).ok()?;
    Some(AndroidFiles {
        app_name: remembered.app_name,
        files: folder.to_path_buf(),
        layout_store: remembered.layout_store,
        image_folder: remembered.image_folder,
        samsung_now_bar: remembered.samsung_now_bar,
    })
}

/// The outcome of a member that this process cannot run: a refusal named by the member's command and key.
#[cfg_attr(
    not(all(feature = "android-push", target_os = "android")),
    allow(
        dead_code,
        reason = "the native entry is Android's, with the feature `android-push`"
    )
)]
fn refused(member: &str, code: ErrorCode) -> String {
    match Wire::read(member) {
        Some(wire) if storable(&wire.activity_key, wire.revision()) => outcome(
            &wire.command,
            &wire.activity_key,
            wire.revision(),
            Err(code),
        ),
        _ => "{}".to_owned(),
    }
}

#[cfg(all(feature = "android-push", target_os = "android"))]
mod native {
    use super::{LiveUpdateBackend, refused, remembered};
    use jni::{
        Env, jni_sig, jni_str,
        objects::{JClass, JObject, JString},
    };
    use liveupdate::ErrorCode;
    use std::{
        ffi::c_void,
        panic::{AssertUnwindSafe, catch_unwind},
        path::{Path, PathBuf},
    };

    /// The entry of the AAR's `LiveUpdatePush.nativeLayer`, exported from the host's library: it runs the member
    /// `liveupdate.layer` through a backend built from `context`, with the files the application's backend last
    /// had, and answers liveupdate's outcome.
    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_io_github_eedaegon_liveupdate_LiveUpdatePush_nativeLayer<
        'caller,
    >(
        mut unowned_env: jni::EnvUnowned<'caller>,
        _class: JClass<'caller>,
        context: JObject<'caller>,
        member: JString<'caller>,
    ) -> JString<'caller> {
        unowned_env
            .with_env(|env| {
                let member = member.mutf8_chars(env)?.to_string();
                let files = files_dir(env, &context)?;
                let vm = env.get_java_vm()?;
                let (vm, raw_context) = (vm.get_raw().cast(), context.as_raw().cast());
                let answer =
                    catch_unwind(AssertUnwindSafe(|| run(vm, raw_context, &files, &member)))
                        .unwrap_or_else(|_| refused(&member, ErrorCode::NativeError));
                env.new_string(answer)
            })
            .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
    }

    /// The folder of the app's files: `context.getFilesDir()`.
    fn files_dir(env: &mut Env<'_>, context: &JObject<'_>) -> jni::errors::Result<PathBuf> {
        let folder = env
            .call_method(
                context,
                jni_str!("getFilesDir"),
                jni_sig!("()Ljava/io/File;"),
                &[],
            )?
            .l()?;
        let path = env
            .call_method(
                &folder,
                jni_str!("getAbsolutePath"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
            .l()?;
        let path = JString::cast_local(env, path)?;
        Ok(PathBuf::from(path.mutf8_chars(env)?.to_string()))
    }

    fn run(vm: *mut c_void, context: *mut c_void, files: &Path, member: &str) -> String {
        let Some(files) = remembered(files) else {
            return refused(member, ErrorCode::NativeError);
        };
        // SAFETY: the process VM, and a context reference that is valid on this thread for the call, as
        // liveupdate's AAR hands them to its own entry.
        #[allow(unsafe_code)]
        let backend = unsafe { LiveUpdateBackend::android_from_raw(vm, context) };
        match backend {
            Ok(backend) => backend.with_files(files).push_layer(member),
            Err(_) => refused(member, ErrorCode::NativeError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AndroidFiles, remember, remembered};

    #[test]
    fn req_a_037_a_backend_remembers_its_files_for_a_push() {
        // REQ-A-037
        let folder =
            std::env::temp_dir().join(format!("liveactivity-remembered-{}", std::process::id()));
        assert_eq!(remembered(&folder), None);
        let files = AndroidFiles {
            samsung_now_bar: true,
            image_folder: "pictures".into(),
            ..AndroidFiles::new("Mochi", &folder)
        };
        remember(&files).unwrap();
        assert_eq!(remembered(&folder), Some(files));
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
