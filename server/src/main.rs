#![no_std]
#![no_main]

//! 回答"这台机器上有哪些设备、谁在驱它们"。
//! 起手一把在 server::serve：收整机物料 → 立账 → 上树 → 逐类立盟 → 落 `/svc/hub` 与 `/dev`
//! → 一枚线程招待所有客人（三面：报名 / 列册 / 认领）。
//! 本镜像由 Driver Cluster 的显式 image build 产出；入口只接公共 `mold`/`execution`
//! 胶水，不依赖 Source 私有 facade。

use hub_server::serve;

#[mold::entry]
fn main() -> Result<(), hub_server::Start> {
    serve::serve()
}
