use crate::renderer::ColorMode;

#[derive(Debug, Clone)]
pub struct UiConfig {
    pub size: usize,
    pub dimensions: usize,
    pub delta: usize,
    pub percent_min: f64,
    pub percent_max: f64,
    pub periodic: bool,
    pub is_playing: bool,
    pub speed_fps: f32,
    pub color_mode: ColorMode,
    pub unlock_extreme_size: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            size: 6,
            dimensions: 4,
            delta: 3,
            percent_min: 20.0,
            percent_max: 45.0,
            periodic: true,
            is_playing: false,
            speed_fps: 5.0,
            color_mode: ColorMode::Hyperdimension,
            unlock_extreme_size: false,
        }
    }
}

/// Returns the maximum recommended edge size for a given dimension.
/// - Safe mode: bounds total cells to <= 3M cells so 100% of cells fit GPU and run smoothly.
/// - Extreme mode: unlocks larger bounds (up to 40 in 6D, up to 2000 in 1D).
pub fn max_size_for_dimensions(dim: usize, unlock_extreme: bool) -> usize {
    if unlock_extreme {
        match dim {
            1 => 2000,
            2 => 1000,
            3 => 200,
            4 => 60,
            5 => 25,
            6 => 40,
            _ => 40,
        }
    } else {
        match dim {
            1 => 1000,
            2 => 300,
            3 => 80,
            4 => 32,
            5 => 16,
            6 => 12,
            _ => 12,
        }
    }
}

/// Information about a saved dump file on disk.
#[derive(Debug, Clone)]
pub struct DumpFileInfo {
    pub path: std::path::PathBuf,
    pub filename: String,
    pub size_bytes: u64,
}

/// Dynamic UI state for dump operations and feedback.
#[derive(Debug, Clone, Default)]
pub struct UiDumpState {
    pub last_status: Option<(String, bool)>,
    pub recent_dumps: Vec<DumpFileInfo>,
}

#[derive(Debug, Default)]
pub struct UiActions {
    pub reconfigure: bool,
    pub randomize: bool,
    pub step: bool,
    pub reset_camera: bool,
    pub switch_dimension: Option<usize>,
    pub toggle_playing: bool,
    pub color_mode_changed: bool,
    pub export_dump_quick: bool,
    pub export_dump_dialog: bool,
    pub import_dump_dialog: bool,
    pub import_dump_path: Option<std::path::PathBuf>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum AppSimStatus {
    #[default]
    Paused,
    Playing,
    Computing,
    Reconfiguring,
}

impl AppSimStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppSimStatus::Paused => "⏸ На паузе (Paused)",
            AppSimStatus::Playing => "▶ Воспроизведение (Playing)",
            AppSimStatus::Computing => "⚙ Вычисление поколения (Computing)...",
            AppSimStatus::Reconfiguring => "🔄 Переконфигурация сетки (Reconfiguring)...",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct UiStats {
    pub generation: u64,
    pub alive_count: usize,
    pub total_cells: usize,
    pub instance_count: usize,
    pub visual_bounds: (usize, usize, usize),
    pub sim_step_time_ms: f64,
    pub fps: f32,
    pub status: AppSimStatus,
}

pub fn render_ui(
    ctx: &egui::Context,
    config: &mut UiConfig,
    stats: &UiStats,
    dump_state: &UiDumpState,
    is_mouse_captured: bool,
) -> UiActions {
    let mut actions = UiActions::default();

    if is_mouse_captured {
        egui::Area::new(egui::Id::new("mouselook_hud"))
            .anchor(egui::Align2::CENTER_TOP, [0.0, 16.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(egui::Color32::from_rgba_premultiplied(12, 18, 30, 230))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 160, 255)))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("🎮 РЕЖИМ КАМЕРЫ")
                                    .color(egui::Color32::from_rgb(100, 220, 255))
                                    .strong(),
                            );
                            ui.label("• Двигайте мышь для поворота");
                            ui.label("• WASD: полёт");
                            ui.label(
                                egui::RichText::new("[Esc]")
                                    .color(egui::Color32::from_rgb(255, 220, 100))
                                    .strong(),
                            );
                            ui.label("вернуть курсор");
                        });
                    });
            });
    }

    egui::Window::new("6D Game of Life Controls")
        .default_pos([16.0, 16.0])
        .default_width(340.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Симуляция 6D Жизни");
            if config.periodic {
                ui.label(
                    egui::RichText::new("🌐 Топология: Периодический n-тор (T^D / PBC)")
                        .color(egui::Color32::from_rgb(100, 200, 255))
                        .small(),
                );
            } else {
                ui.label(
                    egui::RichText::new("⬛ Топология: Ограниченное пространство (Hard Bounds)")
                        .color(egui::Color32::from_rgb(255, 180, 100))
                        .small(),
                );
            }
            ui.label(format!(
                "Поколение: {} | FPS: {:.1}",
                stats.generation, stats.fps
            ));

            ui.horizontal(|ui| {
                ui.label(format!("Статус: {}", stats.status.as_str()));
            });

            if ui
                .checkbox(&mut config.periodic, "🌐 Периодический тор (PBC)")
                .on_hover_text("Включить тороидальное замыкание границ (T^D = S^1 x ... x S^1). Снимите галочку для ограниченного пространства с нулевыми границами.")
                .changed()
            {
                actions.reconfigure = true;
            }
            ui.separator();

            ui.horizontal(|ui| {
                if ui
                    .button(if config.is_playing {
                        "⏸ Пауза (P)"
                    } else {
                        "▶ Пуск (P)"
                    })
                    .clicked()
                {
                    config.is_playing = !config.is_playing;
                    actions.toggle_playing = true;
                }
                if ui.button("⏭ Шаг (Enter)").clicked() {
                    actions.step = true;
                }
                if ui.button("🎲 Рандом (R)").clicked() {
                    actions.randomize = true;
                }
                let can_dump = !config.is_playing;
                if ui
                    .add_enabled(can_dump, egui::Button::new("💾 Дамп"))
                    .on_hover_text(if can_dump {
                        "Создать быстрый дамп текущего состояния в папку dumps/"
                    } else {
                        "Для создания дампа поставьте симуляцию на паузу (P)"
                    })
                    .clicked()
                {
                    actions.export_dump_quick = true;
                }
            });

            ui.add(
                egui::Slider::new(&mut config.speed_fps, 0.5..=60.0)
                    .text("Скорость (шагов/сек)")
                    .logarithmic(true),
            );

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Цвет:");
                let prev_mode = config.color_mode;
                ui.selectable_value(
                    &mut config.color_mode,
                    ColorMode::Hyperdimension,
                    "🌈 Срезы 4D-6D",
                );
                ui.selectable_value(&mut config.color_mode, ColorMode::Uniform, "🟦 Однородный");
                if config.color_mode != prev_mode {
                    actions.color_mode_changed = true;
                }
            });

            if config.color_mode == ColorMode::Hyperdimension && config.dimensions >= 4 {
                ui.collapsing("Легенда гиперкоординат", |ui| match config
                    .dimensions
                {
                    4 => {
                        ui.label("4D: Ось X (Вектор C, 4-е изм.):");
                        ui.label("  • Лазурный -> Зеленый -> Красный");
                    }
                    5 => {
                        ui.label("5D: Сетка срезов X-Y (Векторы B и C):");
                        ui.label("  • Ось X (Вектор C, 4-е изм.): Базовый тон (Hue)");
                        ui.label("  • Ось Y (Вектор B, 5-е изм.): Яркость / насыщенность");
                    }
                    _ => {
                        ui.label("6D: Куб срезов X-Y-Z (Векторы A, B, C):");
                        ui.label("  • Ось X (Вектор C, 4-е изм.) -> +R (Коралл)");
                        ui.label("  • Ось Y (Вектор B, 5-е изм.) -> +G (Изумруд)");
                        ui.label("  • Ось Z (Вектор A, 6-е изм.) -> +B (Сапфир)");
                    }
                });
            }

            ui.separator();
            ui.collapsing("Параметры гиперрешетки", |ui| {
                let mut changed = false;

                let prev_dim = config.dimensions;
                if ui
                    .add(egui::Slider::new(&mut config.dimensions, 1..=6).text("Мерность (1-6)"))
                    .changed()
                {
                    changed = true;
                }

                let max_s = max_size_for_dimensions(config.dimensions, config.unlock_extreme_size);

                // Auto-adapt edge size if dimension changed or current size exceeds max for this dimension
                if config.dimensions != prev_dim {
                    actions.switch_dimension = Some(config.dimensions);
                    if config.size > max_s {
                        config.size = max_s;
                        changed = true;
                    }
                }

                if ui
                    .add(
                        egui::Slider::new(&mut config.size, 1..=max_s)
                            .text(format!("Размерность ребра (1-{})", max_s)),
                    )
                    .changed()
                {
                    changed = true;
                }

                if ui
                    .checkbox(
                        &mut config.unlock_extreme_size,
                        "Разрешить экстремальный размер",
                    )
                    .on_hover_text("Открывает увеличенные пределы размера ребра (до 40 в 6D, до 2000 в 1D)")
                    .changed()
                {
                    let new_max = max_size_for_dimensions(config.dimensions, config.unlock_extreme_size);
                    if config.size > new_max {
                        config.size = new_max;
                    }
                    changed = true;
                }

                // Estimated total cells preview
                let total_preview = (config.size as u128).pow(config.dimensions as u32);
                let (status_text, status_color) = if total_preview <= 1_048_576 {
                    (
                        format!("Оптимально: {} клеток (100% на GPU)", total_preview),
                        egui::Color32::from_rgb(100, 220, 120),
                    )
                } else if total_preview <= 16_777_216 {
                    (
                        format!("Большая сетка: {} клеток (SIMD + выборка GPU)", total_preview),
                        egui::Color32::from_rgb(240, 200, 80),
                    )
                } else {
                    (
                        format!("Экстремально: {} клеток (построчный расчет)", total_preview),
                        egui::Color32::from_rgb(255, 120, 90),
                    )
                };
                ui.colored_label(status_color, status_text);

                if ui
                    .add(
                        egui::Slider::new(&mut config.delta, 0..=8)
                            .text("Зазор между блоками (delta)"),
                    )
                    .changed()
                {
                    changed = true;
                }

                ui.separator();
                if ui
                    .checkbox(
                        &mut config.periodic,
                        "🌐 Периодический n-тор (PBC)",
                    )
                    .on_hover_text("Включить тороидальное замыкание границ (T^D = S^1 x ... x S^1).\nЕсли выключено — ограниченное n-мерное пространство с жесткими/нулевыми границами.")
                    .changed()
                {
                    changed = true;
                }

                ui.separator();
                if ui
                    .add(
                        egui::Slider::new(&mut config.percent_min, 0.0..=100.0)
                            .text("Мин. соседи %"),
                    )
                    .changed()
                {
                    changed = true;
                }
                if ui
                    .add(
                        egui::Slider::new(&mut config.percent_max, 0.0..=100.0)
                            .text("Макс. соседи %"),
                    )
                    .changed()
                {
                    changed = true;
                }

                if changed {
                    actions.reconfigure = true;
                }

                ui.label(
                    egui::RichText::new("⚡ Параметры применяются моментально")
                        .weak()
                        .small(),
                );
            });

            ui.separator();
            ui.collapsing("💾 Дамп состояния (Экспорт / Импорт)", |ui| {
                ui.horizontal(|ui| {
                    if config.is_playing {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 180, 80),
                            "⏸ Нажмите Паузу (P) для создания дампа",
                        );
                    } else {
                        ui.colored_label(
                            egui::Color32::from_rgb(100, 220, 120),
                            "⏸ Симуляция на паузе — сохранение доступно",
                        );
                    }
                });

                ui.add_space(4.0);
                let can_export = !config.is_playing;
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(can_export, egui::Button::new("💾 Быстрый дамп (в dumps/)"))
                        .on_hover_text(if can_export {
                            "Сохранить параметры и клетки в файл с максимальным сжатием (.gol6d)"
                        } else {
                            "Для создания дампа поставьте симуляцию на паузу (P)"
                        })
                        .clicked()
                    {
                        actions.export_dump_quick = true;
                    }

                    if ui
                        .add_enabled(can_export, egui::Button::new("💾 Сохранить как..."))
                        .on_hover_text("Выбрать путь и имя файла для сохранения дампа (.gol6d)")
                        .clicked()
                    {
                        actions.export_dump_dialog = true;
                    }
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui
                        .button("📂 Загрузить файл дампа...")
                        .on_hover_text("Выбрать и загрузить файл дампа (.gol6d) через диалог выбора файлов")
                        .clicked()
                    {
                        actions.import_dump_dialog = true;
                    }
                });

                if let Some((msg, is_err)) = &dump_state.last_status {
                    ui.add_space(4.0);
                    let color = if *is_err {
                        egui::Color32::from_rgb(255, 110, 110)
                    } else {
                        egui::Color32::from_rgb(100, 230, 130)
                    };
                    ui.colored_label(color, msg);
                }

                if !dump_state.recent_dumps.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new("📁 Недавние дампы в dumps/:").strong().small());
                    egui::ScrollArea::vertical()
                        .max_height(140.0)
                        .show(ui, |ui| {
                            for item in &dump_state.recent_dumps {
                                ui.horizontal(|ui| {
                                    if ui.small_button("📥 Загрузить").clicked() {
                                        actions.import_dump_path = Some(item.path.clone());
                                    }
                                    ui.label(
                                        egui::RichText::new(&item.filename)
                                            .monospace()
                                            .small(),
                                    );
                                    let size_str = if item.size_bytes >= 1024 {
                                        format!("{:.1} КБ", item.size_bytes as f64 / 1024.0)
                                    } else {
                                        format!("{} Б", item.size_bytes)
                                    };
                                    ui.label(
                                        egui::RichText::new(format!("({})", size_str))
                                            .weak()
                                            .small(),
                                    );
                                });
                            }
                        });
                }
            });

            ui.separator();
            ui.collapsing("Статистика", |ui| {
                let alive_pct = if stats.total_cells > 0 {
                    (stats.alive_count as f64 / stats.total_cells as f64) * 100.0
                } else {
                    0.0
                };
                ui.label(format!("Всего клеток: {}", stats.total_cells));
                ui.label(format!(
                    "Живых клеток: {} ({:.2}%)",
                    stats.alive_count, alive_pct
                ));
                if stats.instance_count < stats.alive_count {
                    let sample_pct = (stats.instance_count as f64 / stats.alive_count as f64 * 100.0).min(100.0);
                    ui.label(format!(
                        "3D кубов (GPU instances): {} / {} ({:.1}% выборка)",
                        stats.instance_count, stats.alive_count, sample_pct
                    ));
                } else {
                    ui.label(format!(
                        "3D кубов (GPU instances): {} (100% отображение)",
                        stats.instance_count
                    ));
                }
                ui.label(format!(
                    "3D Bounds (X x Y x Z): {} x {} x {}",
                    stats.visual_bounds.0, stats.visual_bounds.1, stats.visual_bounds.2
                ));
                ui.label(format!("Время шага CPU: {:.3} ms", stats.sim_step_time_ms));
            });

            ui.separator();
            ui.collapsing("Управление", |ui| {
                ui.label("🎮 Камера (как в видеоиграх):");
                ui.label("  • Клик вне панели: Вход в режим обзора (курсор скроется)");
                ui.label("  • Движение мыши: Свободный поворот камеры (без зажатия кнопок)");
                ui.label("  • W / S: Вперед / Назад (по направлению взгляда)");
                ui.label("  • A / D: Стрейф влево / вправо");
                ui.label("  • Пробел (Space) / E: Вверх");
                ui.label("  • Shift / Q: Вниз");
                ui.label("  • Esc: Выход из режима обзора и возврат курсора");
                ui.separator();
                ui.label("⚡ Горячие клавиши симуляции:");
                ui.label("  • P: Старт / Пауза (Play/Pause)");
                ui.label("  • Enter / N: 1 шаг симуляции (Step)");
                ui.label("  • R: Случайный посев (Randomize)");
                ui.label("  • C / Home: Сбросить камеру");
                ui.label("  • 1 .. 6: Быстрый выбор мерности (1D-6D)");
                if ui.button("Сбросить камеру (C)").clicked() {
                    actions.reset_camera = true;
                }
            });
        });

    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_max_sizes_per_dimension() {
        // Safe mode guarantees total cells remain within comfortable interactive limits
        for dim in 1..=6 {
            let safe_max = max_size_for_dimensions(dim, false);
            let total_cells = (safe_max as u128).pow(dim as u32);
            assert!(
                total_cells <= 3_500_000,
                "Safe mode for dim {dim} exceeded 3.5M cells: {total_cells}"
            );
        }

        // Exact safe mode limits
        assert_eq!(max_size_for_dimensions(1, false), 1000);
        assert_eq!(max_size_for_dimensions(2, false), 300);
        assert_eq!(max_size_for_dimensions(3, false), 80);
        assert_eq!(max_size_for_dimensions(4, false), 32);
        assert_eq!(max_size_for_dimensions(5, false), 16);
        assert_eq!(max_size_for_dimensions(6, false), 12);

        // Extreme mode limits
        assert_eq!(max_size_for_dimensions(1, true), 2000);
        assert_eq!(max_size_for_dimensions(2, true), 1000);
        assert_eq!(max_size_for_dimensions(3, true), 200);
        assert_eq!(max_size_for_dimensions(4, true), 60);
        assert_eq!(max_size_for_dimensions(5, true), 25);
        assert_eq!(max_size_for_dimensions(6, true), 40);
    }
}
