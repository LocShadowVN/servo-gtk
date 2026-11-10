/// Bộ đệm khung hình chia sẻ 4-slot xoay vòng trên `/dev/shm` (RAM tmpfs)
/// giúp truyền 10MB-20MB pixel RGBA mỗi frame mà chỉ tốn ~40 bytes qua pipe IPC.
struct ShmFrameBuffer {
    slots: [Option<(PathBuf, File)>; 4],
    next_slot: usize,
}

impl ShmFrameBuffer {
    fn new() -> Self {
        let pid = std::process::id();
        let shm_dir = PathBuf::from("/dev/shm");
        if !shm_dir.is_dir() {
            return Self {
                slots: [None, None, None, None],
                next_slot: 0,
            };
        }

        let open_slot = |idx: usize| -> Option<(PathBuf, File)> {
            let path = shm_dir.join(format!("kestrel-frame-{pid}-{idx}.raw"));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .ok()?;
            Some((path, file))
        };

        Self {
            slots: [open_slot(0), open_slot(1), open_slot(2), open_slot(3)],
            next_slot: 0,
        }
    }

    /// Ghi dữ liệu RGBA vào slot tiếp theo và trả về payload `KSHM:<path>` (chỉ vài chục bytes).
    fn write_frame(&mut self, rgba: &[u8]) -> Option<Vec<u8>> {
        let idx = self.next_slot;
        self.next_slot = (self.next_slot + 1) % self.slots.len();

        let (path, file) = self.slots[idx].as_mut()?;
        file.set_len(rgba.len() as u64).ok()?;
        file.seek(SeekFrom::Start(0)).ok()?;
        file.write_all(rgba).ok()?;

        let path_str = path.to_str()?;
        let mut marker = Vec::with_capacity(SHM_FRAME_PREFIX.len() + path_str.len());
        marker.extend_from_slice(SHM_FRAME_PREFIX);
        marker.extend_from_slice(path_str.as_bytes());
        Some(marker)
    }
}

impl Drop for ShmFrameBuffer {
    fn drop(&mut self) {
        for slot in &mut self.slots {
            if let Some((path, _)) = slot.take() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}
