using System;
using System.Drawing;
using System.Globalization;
using System.Windows.Forms;
using MicroTaskPager.Config;

namespace MicroTaskPager.GUI
{
    // Framework side of the settings dialog: the declarative row/tab builders, the render pass that
    // wires change, revert/default and modified state, and the small row model they share. The
    // concrete tabs and options live in SettingsWindow.cs. Kept in the same (partial) class because
    // the builders capture the window's working settings and editing callbacks.
    public partial class SettingsDialog
    {
        // ------------------------------------------------------------------------------------
        // Static helpers
        // ------------------------------------------------------------------------------------

        /// <summary>
        /// Appends an auto-sized row to the given table and returns the new zero-based row index.
        /// </summary>
        /// <returns>The index of the appended row.</returns>
        private static int AddTableRow(TableLayoutPanel table)
        {
            table.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            table.RowCount = table.RowStyles.Count;
            return table.RowCount - 1;
        }

        // ------------------------------------------------------------------------------------
        // Editing state
        // ------------------------------------------------------------------------------------

        private void OnEditorChanged()
        {
            if (_initializing || _suppressEvents)
            {
                return;
            }

            RefreshRevertButtons();
            RequestPreview();
        }

        private void Suppress(Action action)
        {
            bool previous = _suppressEvents;
            _suppressEvents = true;
            try
            {
                action();
            }
            finally
            {
                _suppressEvents = previous;
            }
        }

        private void RefreshRevertButtons()
        {
            foreach (RowBinding row in _rows)
            {
                row.Revert.Visible = row.IsModified();
            }
        }

        private void RequestPreview()
        {
            if (_initializing || _suppressEvents)
            {
                return;
            }

            // Coalesce rapid changes (e.g. holding a NumericUpDown arrow) so the pager is not
            // rebuilt and retopmosted for every single increment.
            _previewTimer.Stop();
            _previewTimer.Start();
        }

        private void FlushPreview()
        {
            if (_closing || _initializing || _suppressEvents)
            {
                return;
            }

            AppSettings preview = TryBuildSettings(false);
            if (preview != null)
            {
                PreviewChanged?.Invoke(preview);
            }
        }

        // ------------------------------------------------------------------------------------
        // Formatting helpers
        // ------------------------------------------------------------------------------------

        private string FormatOpacity(double opacity)
        {
            return opacity.ToString("0.00", CultureInfo.InvariantCulture);
        }

        private Color PickColor(Color initial)
        {
            using (var dialog = new ColorDialog())
            {
                dialog.FullOpen = true;
                dialog.Color = initial;
                return dialog.ShowDialog() == DialogResult.OK ? dialog.Color : initial;
            }
        }

        // ------------------------------------------------------------------------------------
        // Row rendering
        // ------------------------------------------------------------------------------------

        /// <summary>
        /// Renders the given rows into the group in order, adding a label, the row's host control and
        /// a revert slot per row, and registering each row against the group's page so revert and the
        /// per-tab Defaults action can reach it. All change, load and modified wiring lives here, so
        /// rows cannot drift apart.
        /// </summary>
        private void Render(RowGroup group, params RowSpec[] specs)
        {
            TabPage page = group.Page;
            TableLayoutPanel table = group.Table;

            foreach (RowSpec spec in specs)
            {
                int row = AddTableRow(table);

                var label = new Label
                {
                    Text = spec.Label,
                    AutoSize = true,
                    Anchor = AnchorStyles.Left,
                    Margin = new Padding(6, 9, 6, 6),
                };

                if (!string.IsNullOrEmpty(spec.ToolTip))
                {
                    _toolTip.SetToolTip(label, spec.ToolTip);
                }

                spec.Host.Anchor = AnchorStyles.Left;
                spec.Host.Margin = new Padding(3, 5, 3, 5);

                var revert = new Button
                {
                    Text = "\u21BA",
                    Size = new Size(28, 24),
                    Location = new Point(0, 2),
                    Visible = spec.IsModified(),
                };
                _toolTip.SetToolTip(revert, "Revert to default");
                revert.Click += (s, e) =>
                {
                    Suppress(() => spec.Load(_settingsBefore));
                    RefreshRevertButtons();
                    RequestPreview();
                };

                // The host keeps the revert slot reserved even while the button is hidden,
                // otherwise WinForms would reflow the whole row on every visibility change.
                var revertHost = new Panel
                {
                    Size = new Size(32, 28),
                    Margin = new Padding(4, 4, 0, 4),
                    Anchor = AnchorStyles.Left,
                };
                revertHost.Controls.Add(revert);

                table.Controls.Add(label, 0, row);
                table.Controls.Add(spec.Host, 1, row);
                table.Controls.Add(revertHost, 2, row);

                _rows.Add(new RowBinding(page, revert, spec.Load, spec.IsModified));
            }
        }

        /// <summary>
        /// Builds a row from a single int-valued editor such as a combo selection or numeric value.
        /// Label and toolTip are the row caption and optional hover text; the host control is placed
        /// in the middle column. Read and show move the value between the editor and its stored form,
        /// get and set move it between a settings snapshot and the working copy, and subscribe
        /// registers the editor's change event so edits flow into the working settings.
        /// </summary>
        /// <returns>A row spec that also loads revert/default values and tracks modification state.</returns>
        private RowSpec LinkedRow(
            string label,
            string toolTip,
            Control editor,
            Func<int> read,
            Action<int> show,
            Action<Action> subscribe,
            Func<AppSettings, int> get,
            Action<AppSettings, int> set)
        {
            subscribe(() =>
            {
                set(_working, read());
                OnEditorChanged();
            });

            return new RowSpec(
                editor,
                label,
                toolTip,
                s =>
                {
                    int value = get(s);
                    show(value);
                    set(_working, value);
                },
                () => read() != get(_settingsBefore));
        }

        /// <summary>
        /// Creates a drop-down row. Label is the caption, items define both the entries and the
        /// stored index order, and get/set map that index to and from the settings. The optional
        /// toolTip adds hover text to the label.
        /// </summary>
        /// <returns>The row spec for the drop-down.</returns>
        private RowSpec ComboRow(
            string label,
            string[] items,
            Func<AppSettings, int> get,
            Action<AppSettings, int> set,
            string toolTip = null)
        {
            var editor = new ComboBox
            {
                DropDownStyle = ComboBoxStyle.DropDownList,
                Width = 180,
            };
            editor.Items.AddRange(items);
            editor.SelectedIndex = get(_working);

            return LinkedRow(
                label,
                toolTip,
                editor,
                () => editor.SelectedIndex,
                index => editor.SelectedIndex = index,
                handler => editor.SelectedIndexChanged += (s, e) => handler(),
                get,
                set);
        }

        /// <summary>
        /// Creates a numeric row. Min and max clamp the editor, and get/set map the value to and from
        /// the settings. The optional toolTip adds hover text to the label, and the optional normalize
        /// callback adjusts a freshly entered value (for a dynamic lower bound) before it is written
        /// back.
        /// </summary>
        /// <returns>The row spec for the numeric editor.</returns>
        private RowSpec NumericRow(
            string label,
            int min,
            int max,
            Func<AppSettings, int> get,
            Action<AppSettings, int> set,
            string toolTip = null,
            Func<int, int> normalize = null)
        {
            var editor = new NumericUpDown
            {
                Minimum = min,
                Maximum = max,
                Width = 150,
                TextAlign = HorizontalAlignment.Right,
                Value = Math.Max(min, Math.Min(max, get(_working))),
            };
            editor.ValueChanged += (s, e) =>
            {
                int value = (int)editor.Value;
                if (normalize != null)
                {
                    int normalized = Math.Max(min, Math.Min(max, normalize(value)));
                    if (normalized != value)
                    {
                        value = normalized;
                        Suppress(() => editor.Value = value);
                    }
                }

                set(_working, value);
                OnEditorChanged();
            };

            return new RowSpec(
                editor,
                label,
                toolTip,
                s =>
                {
                    editor.Value = Math.Max(min, Math.Min(max, get(s)));
                    set(_working, (int)editor.Value);
                },
                () => (int)editor.Value != get(_settingsBefore));
        }

        /// <summary>
        /// Creates a slider row with a companion value label. Min and max bound the slider position,
        /// and getPosition/setPosition translate between a settings value and that integer position.
        /// Format renders the position for the value label, and isModified decides whether a position
        /// differs from the initial settings. The optional toolTip adds hover text to the label;
        /// tickFrequency and largeChange default to single steps.
        /// </summary>
        /// <returns>The row spec for the slider.</returns>
        private RowSpec SliderRow(
            string label,
            int min,
            int max,
            Func<AppSettings, int> getPosition,
            Action<AppSettings, int> setPosition,
            Func<int, string> format,
            Func<int, bool> isModified,
            string toolTip = null,
            int tickFrequency = 1,
            int largeChange = 1)
        {
            var trackBar = new TrackBar
            {
                Minimum = min,
                Maximum = max,
                TickFrequency = tickFrequency,
                LargeChange = largeChange,
                SmallChange = 1,
                Value = Math.Max(min, Math.Min(max, getPosition(_working))),
                AutoSize = false,
                Width = 140,
                Height = 32,
                Margin = new Padding(0, 6, 4, 0),
            };

            var valueLabel = new Label
            {
                AutoSize = true,
                Text = format(trackBar.Value),
                Margin = new Padding(4, 14, 0, 0),
            };

            var host = new FlowLayoutPanel
            {
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                WrapContents = false,
                Margin = new Padding(0),
            };
            host.Controls.Add(trackBar);
            host.Controls.Add(valueLabel);

            trackBar.ValueChanged += (s, e) =>
            {
                setPosition(_working, trackBar.Value);
                valueLabel.Text = format(trackBar.Value);
                OnEditorChanged();
            };

            return new RowSpec(
                host,
                label,
                toolTip,
                s =>
                {
                    int position = Math.Max(min, Math.Min(max, getPosition(s)));
                    trackBar.Value = position;
                    valueLabel.Text = format(position);
                    setPosition(_working, position);
                },
                () => isModified(trackBar.Value));
        }

        /// <summary>
        /// Creates a boolean row. Get and set read and write the flag in the settings, and the
        /// optional toolTip adds hover text to the label. Set enabled to false for options that
        /// are not available on the current Windows version.
        /// </summary>
        /// <returns>The row spec for the check box.</returns>
        private RowSpec CheckBoxRow(
            string label,
            Func<AppSettings, bool> get,
            Action<AppSettings, bool> set,
            string toolTip = null,
            bool enabled = true)
        {
            var editor = new CheckBox
            {
                AutoSize = true,
                Checked = get(_working),
                Enabled = enabled,
            };
            editor.CheckedChanged += (s, e) =>
            {
                set(_working, editor.Checked);
                OnEditorChanged();
            };

            return new RowSpec(
                editor,
                label,
                toolTip,
                s =>
                {
                    editor.Checked = get(s);
                    set(_working, editor.Checked);
                },
                () => editor.Checked != get(_settingsBefore));
        }

        /// <summary>
        /// Creates an integer text row. Get and set read and write the value, and the optional
        /// toolTip adds hover text to the label. Unparsable text leaves the working setting unchanged
        /// and marks the row as modified so Save reports a validation error.
        /// </summary>
        /// <returns>The row spec for the text box.</returns>
        private RowSpec TextRow(
            string label,
            Func<AppSettings, int> get,
            Action<AppSettings, int> set,
            string toolTip = null)
        {
            var editor = new TextBox
            {
                Width = 150,
                TextAlign = HorizontalAlignment.Right,
            };
            editor.Text = get(_working).ToString(CultureInfo.InvariantCulture);
            editor.TextChanged += (s, e) =>
            {
                if (int.TryParse(editor.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out int value))
                {
                    set(_working, value);
                }

                OnEditorChanged();
            };

            return new RowSpec(
                editor,
                label,
                toolTip,
                s => editor.Text = get(s).ToString(CultureInfo.InvariantCulture),
                () => !int.TryParse(editor.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out int value)
                    || value != get(_settingsBefore));
        }

        /// <summary>
        /// Creates a row whose editor is a button that runs the given action. The row never counts
        /// as modified, so it has no revert behavior.
        /// </summary>
        /// <returns>The row spec for the button.</returns>
        private RowSpec ButtonRow(
            string label,
            string text,
            Action onClick,
            string toolTip = null)
        {
            var editor = new Button
            {
                Text = text,
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                Padding = new Padding(8, 2, 8, 2),
            };
            editor.Click += (s, e) => onClick();

            return new RowSpec(editor, label, toolTip, s => { }, () => false);
        }

        /// <summary>
        /// Creates a color row with a swatch and a picker button. Get and set read and write the
        /// color in the settings, and the optional toolTip adds hover text to the label.
        /// </summary>
        /// <returns>The row spec for the color swatch and picker.</returns>
        private RowSpec ColorRow(
            string label,
            Func<AppSettings, Color> get,
            Action<AppSettings, Color> set,
            string toolTip = null)
        {
            var preview = new Panel
            {
                Size = new Size(32, 24),
                BorderStyle = BorderStyle.FixedSingle,
                BackColor = get(_working),
                Margin = new Padding(0, 4, 6, 0),
            };

            var choose = new Button
            {
                Text = "Choose...",
                Size = new Size(80, 24),
                Margin = new Padding(0, 4, 0, 0),
            };
            choose.Click += (s, e) =>
            {
                Color picked = PickColor(get(_working));
                if (picked.ToArgb() == get(_working).ToArgb())
                {
                    return;
                }

                set(_working, picked);
                preview.BackColor = picked;
                OnEditorChanged();
            };

            var host = new FlowLayoutPanel
            {
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                WrapContents = false,
                Margin = new Padding(0),
            };
            host.Controls.Add(preview);
            host.Controls.Add(choose);

            return new RowSpec(
                host,
                label,
                toolTip,
                s =>
                {
                    set(_working, get(s));
                    preview.BackColor = get(s);
                },
                () => get(_working).ToArgb() != get(_settingsBefore).ToArgb());
        }

        // ------------------------------------------------------------------------------------
        // Layout helpers
        // ------------------------------------------------------------------------------------

        /// <summary>
        /// Appends a row to the given table whose control spans all three columns, e.g. the pinning
        /// note box.
        /// </summary>
        private void AddSpanningControl(TableLayoutPanel table, Control control)
        {
            int row = AddTableRow(table);
            control.Dock = DockStyle.Fill;
            table.Controls.Add(control, 0, row);
            table.SetColumnSpan(control, 3);
        }

        /// <summary>
        /// Creates the empty shell of a settings tab titled with the given caption: a scrolling page
        /// with a one-column stack. The stack is kept in the page tag for BeginGroup.
        /// </summary>
        /// <returns>The prepared tab page.</returns>
        private TabPage CreateTabPage(string title)
        {
            var page = new TabPage(title);
            var scroll = new Panel
            {
                Dock = DockStyle.Fill,
                AutoScroll = true,
            };

            var stack = new TableLayoutPanel
            {
                Dock = DockStyle.Top,
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                ColumnCount = 1,
                Padding = new Padding(2),
            };
            stack.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

            scroll.Controls.Add(stack);
            page.Controls.Add(scroll);
            page.Tag = stack;
            return page;
        }

        /// <summary>
        /// Adds a group box with the given caption to the page's stack and returns its page and
        /// three-column table, which is the target for Render.
        /// </summary>
        /// <returns>The group's page and table.</returns>
        private RowGroup BeginGroup(TabPage page, string title)
        {
            var stack = (TableLayoutPanel)page.Tag;
            var group = new GroupBox
            {
                Text = title,
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                Dock = DockStyle.Top,
                Margin = new Padding(6, 4, 6, 4),
            };

            var table = new TableLayoutPanel
            {
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                Dock = DockStyle.Top,
                ColumnCount = 3,
                Padding = new Padding(10, 10, 10, 10),
            };
            table.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 200));
            table.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 190));
            table.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 36));

            group.Controls.Add(table);

            int row = AddTableRow(stack);
            stack.Controls.Add(group, 0, row);

            return new RowGroup(page, table);
        }

        // ------------------------------------------------------------------------------------
        // Row model
        // ------------------------------------------------------------------------------------

        /// <summary>
        /// A group's tab page and its three-column table, so Render does not need the page passed
        /// separately. Created by BeginGroup.
        /// </summary>
        private sealed class RowGroup
        {
            /// <summary>
            /// Initializes a new instance of the <see cref="RowGroup"/> class.
            /// Initializes a group binding. Page owns the tab the rows belong to, and table is the
            /// three-column target the rows are added to.
            /// </summary>
            public RowGroup(TabPage page, TableLayoutPanel table)
            {
                Page = page;
                Table = table;
            }

            public TabPage Page { get; }

            public TableLayoutPanel Table { get; }
        }

        /// <summary>
        /// A single settings row in declarative form: the host control plus its label, reset behavior
        /// and modification check that Render turns into a table row. Created by the row factories.
        /// </summary>
        private sealed class RowSpec
        {
            /// <summary>
            /// Initializes a new instance of the <see cref="RowSpec"/> class.
            /// Initializes a row description. Host is the control placed in the middle column, label
            /// the caption, and toolTip the optional hover text. Load applies a settings snapshot to
            /// the host and working state, while isModified reports whether the row differs from the
            /// initial settings.
            /// </summary>
            public RowSpec(
                Control host,
                string label,
                string toolTip,
                Action<AppSettings> load,
                Func<bool> isModified)
            {
                Host = host;
                Label = label;
                ToolTip = toolTip;
                Load = load;
                IsModified = isModified;
            }

            public Control Host { get; }

            public string Label { get; }

            public string ToolTip { get; }

            public Action<AppSettings> Load { get; }

            public Func<bool> IsModified { get; }
        }

        /// <summary>
        /// A rendered row, remembered so the revert buttons and the per-tab Defaults action can
        /// reach it after construction.
        /// </summary>
        private sealed class RowBinding
        {
            /// <summary>
            /// Initializes a new instance of the <see cref="RowBinding"/> class.
            /// Initializes a rendered-row binding. Page is the tab the row belongs to, revert is its
            /// button, load applies a settings snapshot, and isModified reports whether the row
            /// differs from the initial settings.
            /// </summary>
            public RowBinding(TabPage page, Button revert, Action<AppSettings> load, Func<bool> isModified)
            {
                Page = page;
                Revert = revert;
                Load = load;
                IsModified = isModified;
            }

            public TabPage Page { get; }

            public Button Revert { get; }

            public Action<AppSettings> Load { get; }

            public Func<bool> IsModified { get; }
        }
    }
}
