# 千实例性能基线

记录日期：2026-07-14。环境：Apple Silicon macOS，本地 debug 测试构建，1000 个独立 Skill 实例。

| 场景 | 自动化门槛 | 本次记录 |
| --- | ---: | ---: |
| 全文检索 + 客户端/状态筛选 + 排序，连续 30 次 | 总计 < 3 秒 | 约 67 毫秒，平均约 2.23 毫秒 |
| 重复检查结果生成 | < 5 秒 | 约 0.35 秒（优化前约 7.15 秒） |
| 前端 1000 个结果按需展示、筛选、排序、选择及检索到单行 | 全流程 < 5 秒，单次 React commit < 1 秒 | 测试体约 0.58 秒 |

执行方式：

```bash
cargo test -p skill-workspace --test search_catalog thousand_instance_catalog_keeps_common_search_and_filter_within_baseline -- --nocapture
cargo test -p skill-workspace --test duplicate_review thousand_instance_duplicate_review_stays_within_recorded_baseline -- --nocapture
npm test -- --run src/App.test.tsx -t 千实例列表
```

这些门槛是首版回归警戒线，不是跨机器的绝对性能承诺。测试使用真实 SQLite 索引和真实临时目录；前端测试覆盖首批 250 个结果、按需显示更多、勾选反馈、完整结果集筛选与检索收敛。重复检查在 Tauri 阻塞任务线程中执行，不占用浏览器主线程。
