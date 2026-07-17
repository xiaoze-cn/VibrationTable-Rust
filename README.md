# FP200

柔性振动台 Rust SDK

## 安装

```toml
[dependencies]
flexible_vibration_table = { git = "https://github.com/xiaoze-cn/FP200.git" }
```

## 示例

常用设备地址为 `192.168.3.8:8887` 或 `192.168.3.7:8887`

```rust
use flexible_vibration_table::{MotionMode, VibrationTable};

fn main() -> flexible_vibration_table::Result<()> {
    let mut table = VibrationTable::connect("192.168.3.8:8887")?;

    table.clear_faults()?;
    table.light_on(200)?;
    table.start_vibration(MotionMode::MoveForward)?;
    table.stop_vibration()?;
    table.light_off()?;

    Ok(())
}
```
