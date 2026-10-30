using System;
using System.Collections.Generic;
using System.Drawing;
using System.Globalization;
using System.Windows.Forms;
using MicroTaskPager.Config;

namespace MicroTaskPager.GUI
{
    // Settings dialog built entirely in code. All rows are created through a few helpers so that
    // layout, revert buttons, "Defaults" handling and live preview behave identically everywhere.
    public partial class AppSettingsWindow : Form
    {
        private const int PlacementGap = 8;
        private const int MinPagerContentHeight = 16;

        private readonly AppSettings _defaults = new AppSettings();
        private readonly AppSettings _settingsBefore;
        private readonly AppSettings _working;
        private readonly List<RowBinding> _rows = new List<RowBinding>();
        private readonly ToolTip _toolTip = new ToolTip();
        private readonly Timer _previewTimer = new Timer { Interval = 80 };

        private TabControl _tabs;
        private TextBox _primaryDelayBox;
        private TextBox _secondaryDelayBox;
        private Rectangle _placementOwnerBounds;
        private bool _hasPlacementOwner;
        private bool _initializing = true;
        private bool _suppressEvents;
        private bool _closing;

        public AppSettingsWindow(AppSettings settings, Rectangle ownerBounds)
        {
            _settingsBefore = AppSettings.Clone(settings);
            _working = AppSettings.Clone(settings);

            InitializeComponent();

            _placementOwnerBounds = ownerBounds;
            _hasPlacementOwner = true;

            _previewTimer.Tick += (s, e) =>
            {
                _previewTimer.Stop();
                FlushPreview();
            };

            _initializing = false;
            RefreshRevertButtons();
        }

        public event Action<AppSettings> PreviewChanged;

        public AppSettings Settings { get; private set; }

        protected override void OnLoad(EventArgs e)
        {
            base.OnLoad(e);

            if (_hasPlacementOwner)
            {
                Location = FindPlacement(_placementOwnerBounds, Size);
            }
        }

        protected override void OnFormClosing(FormClosingEventArgs e)
        {
            // A queued preview must not be applied after the dialog was cancelled.
            _closing = true;
            _previewTimer.Stop();
            base.OnFormClosing(e);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                _previewTimer.Dispose();
                _toolTip.Dispose();
            }

            base.Dispose(disposing);
        }

        // ------------------------------------------------------------------------------------
        // Row plumbing
        // ------------------------------------------------------------------------------------

        private static int AddTableRow(TableLayoutPanel table)
        {
            table.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            table.RowCount = table.RowStyles.Count;
            return table.RowCount - 1;
        }

        private void AddRow(
            TableLayoutPanel table,
            TabPage page,
            string labelText,
            string toolTipText,
            Control editor,
            Action<AppSettings> load,
            Func<bool> isModified)
        {
            int row = AddTableRow(table);

            var label = new Label
            {
                Text = labelText,
                AutoSize = true,
                Anchor = AnchorStyles.Left,
                Margin = new Padding(6, 9, 6, 6),
            };

            if (!string.IsNullOrEmpty(toolTipText))
            {
                _toolTip.SetToolTip(label, toolTipText);
            }

            editor.Anchor = AnchorStyles.Left;
            editor.Margin = new Padding(3, 5, 3, 5);

            var revert = new Button
            {
                Text = "\u21BA",
                Size = new Size(28, 24),
                Location = new Point(0, 2),
                Visible = isModified(),
            };
            _toolTip.SetToolTip(revert, "Revert to default");
            revert.Click += (s, e) =>
            {
                Suppress(() => load(_settingsBefore));
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
            table.Controls.Add(editor, 1, row);
            table.Controls.Add(revertHost, 2, row);

            _rows.Add(new RowBinding(page, revert, load, isModified));
        }

        private void AddSpanningControl(TableLayoutPanel table, Control control)
        {
            int row = AddTableRow(table);
            control.Dock = DockStyle.Fill;
            table.Controls.Add(control, 0, row);
            table.SetColumnSpan(control, 3);
        }

        private void AddComboRow(
            TableLayoutPanel table,
            TabPage page,
            string label,
            string toolTip,
            string[] items,
            Func<AppSettings, int> get,
            Action<AppSettings, int> set)
        {
            var editor = new ComboBox
            {
                DropDownStyle = ComboBoxStyle.DropDownList,
                Width = 180,
            };
            editor.Items.AddRange(items);
            editor.SelectedIndex = get(_working);
            editor.SelectedIndexChanged += (s, e) =>
            {
                set(_working, editor.SelectedIndex);
                OnEditorChanged();
            };

            AddRow(
                table,
                page,
                label,
                toolTip,
                editor,
                s => editor.SelectedIndex = get(s),
                () => editor.SelectedIndex != get(_settingsBefore));
        }

        private void AddNumericRow(
            TableLayoutPanel table,
            TabPage page,
            string label,
            string toolTip,
            int min,
            int max,
            Func<AppSettings, int> get,
            Action<AppSettings, int> set)
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
                set(_working, (int)editor.Value);
                OnEditorChanged();
            };

            AddRow(
                table,
                page,
                label,
                toolTip,
                editor,
                s => editor.Value = Math.Max(min, Math.Min(max, get(s))),
                () => (int)editor.Value != get(_settingsBefore));
        }

        private TextBox AddTextRow(
            TableLayoutPanel table,
            TabPage page,
            string label,
            string toolTip,
            Func<AppSettings, int> get,
            Action<AppSettings, int> set)
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

            AddRow(
                table,
                page,
                label,
                toolTip,
                editor,
                s => editor.Text = get(s).ToString(CultureInfo.InvariantCulture),
                () => !int.TryParse(editor.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out int value)
                    || value != get(_settingsBefore));

            return editor;
        }

        private void AddCheckBoxRow(
            TableLayoutPanel table,
            TabPage page,
            string label,
            string toolTip,
            Func<AppSettings, bool> get,
            Action<AppSettings, bool> set)
        {
            var editor = new CheckBox
            {
                AutoSize = true,
                Checked = get(_working),
            };
            editor.CheckedChanged += (s, e) =>
            {
                set(_working, editor.Checked);
                OnEditorChanged();
            };

            AddRow(
                table,
                page,
                label,
                toolTip,
                editor,
                s => editor.Checked = get(s),
                () => editor.Checked != get(_settingsBefore));
        }

        private void AddColorRow(
            TableLayoutPanel table,
            TabPage page,
            string label,
            Func<AppSettings, Color> get,
            Action<AppSettings, Color> set)
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

            AddRow(
                table,
                page,
                label,
                null,
                host,
                s =>
                {
                    set(_working, get(s));
                    preview.BackColor = get(s);
                },
                () => get(_working).ToArgb() != get(_settingsBefore).ToArgb());
        }

        private void AddOpacityRow(TableLayoutPanel table, TabPage page)
        {
            var trackBar = new TrackBar
            {
                Minimum = 25,
                Maximum = 100,
                TickFrequency = 5,
                LargeChange = 10,
                SmallChange = 1,
                Value = Math.Max(25, Math.Min(100, (int)Math.Round(_working.BackgroundOpacity * 100))),
                AutoSize = false,
                Width = 140,
                Height = 32,
                Margin = new Padding(0, 6, 4, 0),
            };

            var valueLabel = new Label
            {
                AutoSize = true,
                Text = FormatOpacity(trackBar.Value / 100.0),
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
                _working.BackgroundOpacity = trackBar.Value / 100.0;
                valueLabel.Text = FormatOpacity(trackBar.Value / 100.0);
                OnEditorChanged();
            };

            AddRow(
                table,
                page,
                "App Background Opacity",
                null,
                host,
                s =>
                {
                    trackBar.Value = Math.Max(25, Math.Min(100, (int)Math.Round(s.BackgroundOpacity * 100)));
                    valueLabel.Text = FormatOpacity(trackBar.Value / 100.0);
                    _working.BackgroundOpacity = trackBar.Value / 100.0;
                },
                () => Math.Abs((trackBar.Value / 100.0) - _settingsBefore.BackgroundOpacity) > 0.0001);
        }

        // ------------------------------------------------------------------------------------
        // State changes
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

        private AppSettings TryBuildSettings(bool showValidationErrors)
        {
            if (!int.TryParse(
                    GetPrimaryDelayText(),
                    NumberStyles.Integer,
                    CultureInfo.InvariantCulture,
                    out int primaryDelay)
                || primaryDelay < 1)
            {
                if (showValidationErrors)
                {
                    ShowValidationError("Primary Update Delay must be at least 1 ms.");
                }

                return null;
            }

            if (!int.TryParse(
                    GetSecondaryDelayText(),
                    NumberStyles.Integer,
                    CultureInfo.InvariantCulture,
                    out int secondaryDelay)
                || secondaryDelay < 100)
            {
                if (showValidationErrors)
                {
                    ShowValidationError("Secondary Update Delay must be at least 100 ms.");
                }

                return null;
            }

            AppSettings result = AppSettings.Clone(_working);
            result.PrimaryUpdateDelay = primaryDelay;
            result.SecondaryUpdateDelay = secondaryDelay;
            return result;
        }

        private string GetPrimaryDelayText() => _primaryDelayBox.Text;

        private string GetSecondaryDelayText() => _secondaryDelayBox.Text;

        private void ShowValidationError(string message)
        {
            MessageBox.Show(this, message, "Invalid Settings", MessageBoxButtons.OK, MessageBoxIcon.Warning);
        }

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

        private void Save_Click(object sender, EventArgs e)
        {
            AppSettings settings = TryBuildSettings(true);
            if (settings == null)
            {
                return;
            }

            Settings = settings;
            DialogResult = DialogResult.OK;
            Close();
        }

        private void Cancel_Click(object sender, EventArgs e)
        {
            DialogResult = DialogResult.Cancel;
            Close();
        }

        private void Defaults_Click(object sender, EventArgs e)
        {
            TabPage page = _tabs.SelectedTab;
            foreach (RowBinding row in _rows)
            {
                if (row.Page == page)
                {
                    Suppress(() => row.Load(_defaults));
                }
            }

            RefreshRevertButtons();
            RequestPreview();
        }

        // ------------------------------------------------------------------------------------
        // UI construction
        // ------------------------------------------------------------------------------------

        private void InitializeComponent()
        {
            SuspendLayout();

            // Pixel layout is authored at 96 DPI; let WinForms scale it like AboutDialog does.
            AutoScaleDimensions = new SizeF(96F, 96F);
            AutoScaleMode = AutoScaleMode.Dpi;

            Text = "Micro Task Pager Settings";
            ClientSize = new Size(520, 640);
            StartPosition = FormStartPosition.Manual;
            FormBorderStyle = FormBorderStyle.FixedDialog;
            MaximizeBox = false;
            MinimizeBox = false;
            ShowInTaskbar = false;

            // The pager itself is topmost. Keeping the modal settings dialog topmost as well
            // prevents the pager from being raised above it and stealing the foreground while
            // the preview is updated.
            TopMost = true;
            Padding = new Padding(10, 10, 10, 0);

            _tabs = new TabControl
            {
                Dock = DockStyle.Fill,
            };

            TabPage layoutPage = CreatePage("Layout");
            TabPage themePage = CreatePage("Theme");
            TabPage miscPage = CreatePage("Misc");

            _tabs.TabPages.Add(layoutPage);
            _tabs.TabPages.Add(themePage);
            _tabs.TabPages.Add(miscPage);

            BuildLayoutPage(layoutPage);
            BuildThemePage(themePage);
            BuildMiscPage(miscPage);

            var buttons = new FlowLayoutPanel
            {
                Dock = DockStyle.Bottom,
                FlowDirection = FlowDirection.RightToLeft,
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                WrapContents = false,
                Padding = new Padding(0, 8, 0, 0),
            };

            var cancel = new Button { Text = "Cancel", Width = 100, Height = 28, Margin = new Padding(8, 0, 0, 0) };
            cancel.Click += Cancel_Click;

            var save = new Button { Text = "Save", Width = 100, Height = 28, Margin = new Padding(8, 0, 0, 0) };
            save.Click += Save_Click;

            var defaults = new Button { Text = "Defaults", Width = 100, Height = 28, Margin = new Padding(0) };
            _toolTip.SetToolTip(defaults, "Reset for current tab only");
            defaults.Click += Defaults_Click;

            buttons.Controls.Add(cancel);
            buttons.Controls.Add(save);
            buttons.Controls.Add(defaults);

            AcceptButton = save;
            CancelButton = cancel;

            Controls.Add(_tabs);
            Controls.Add(buttons);

            ResumeLayout(false);
        }

        private TabPage CreatePage(string title)
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

        private TableLayoutPanel BeginGroup(TabPage page, string title)
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

            return table;
        }

        private void BuildLayoutPage(TabPage page)
        {
            TableLayoutPanel rendering = BeginGroup(page, "Rendering");

            AddComboRow(
                rendering,
                page,
                "View Mode Default",
                "Only affects newly created desktops and their screens.",
                new[] { "Preview", "MicroTaskbar" },
                s => (int)s.ViewModeDefault,
                (s, v) => s.ViewModeDefault = (ViewMode)v);

            AddPagerHeightRow(rendering, page);

            TableLayoutPanel icons = BeginGroup(page, "Icons Settings");

            AddNumericRow(
                icons,
                page,
                "Icon Padding X",
                "Negative values are supported as well so less space is taken and the icons look stacked.",
                -9999,
                9999,
                s => s.IconPaddingX,
                (s, v) => s.IconPaddingX = v);

            AddNumericRow(
                icons,
                page,
                "Icon Padding Y",
                "Negative values are supported as well in case of a very small pager height, like 40px which is usually the size of the Windows Taskbar.",
                -9999,
                9999,
                s => s.IconPaddingY,
                (s, v) => s.IconPaddingY = v);
        }

        private void AddPagerHeightRow(TableLayoutPanel table, TabPage page)
        {
            var editor = new NumericUpDown
            {
                Minimum = MinPagerContentHeight,
                Maximum = 9999,
                Width = 150,
                TextAlign = HorizontalAlignment.Right,
            };
            editor.Value = Math.Max(editor.Minimum, Math.Min(editor.Maximum, _working.PagerHeight));

            editor.ValueChanged += (s, e) =>
            {
                int value = (int)editor.Value;
                int minimum = _working.IconPaddingY + MinPagerContentHeight;
                if (value < minimum)
                {
                    value = (int)Math.Min(editor.Maximum, minimum);
                    Suppress(() => editor.Value = value);
                }

                _working.PagerHeight = value;
                OnEditorChanged();
            };

            AddRow(
                table,
                page,
                "Pager Height",
                "The pager's width is automatically calculated from this value and from the size of all available screen space as well as the amount of virtual desktops.",
                editor,
                s => editor.Value = Math.Max(editor.Minimum, Math.Min(editor.Maximum, s.PagerHeight)),
                () => (int)editor.Value != _settingsBefore.PagerHeight);
        }

        private void BuildThemePage(TabPage page)
        {
            TableLayoutPanel general = BeginGroup(page, "General");
            AddOpacityRow(general, page);

            TableLayoutPanel desktops = BeginGroup(page, "Desktops");
            AddComboRow(desktops, page, "Border Style", null, new[] { "Box", "Underline" }, s => (int)s.DesktopBorderStyle,
                (s, v) => s.DesktopBorderStyle = (Config.DesktopBorderStyle)v);
            AddColorRow(desktops, page, "Background Color", s => s.DesktopBackgroundColor,
                (s, c) => s.DesktopBackgroundColor = c);
            AddColorRow(desktops, page, "Border Color", s => s.DesktopBorderColor,
                (s, c) => s.DesktopBorderColor = c);
            AddColorRow(desktops, page, "Border Color (Active)", s => s.ActiveDesktopBorderColor,
                (s, c) => s.ActiveDesktopBorderColor = c);

            TableLayoutPanel windows = BeginGroup(page, "Preview Windows");
            AddColorRow(windows, page, "Background Color", s => s.WindowBackgroundColor, (s, c) => s.WindowBackgroundColor = c);
            AddColorRow(windows, page, "Border Color", s => s.WindowBorderColor, (s, c) => s.WindowBorderColor = c);
            AddColorRow(windows, page, "Border Color (Active)", s => s.ActiveWindowBorderColor, (s, c) => s.ActiveWindowBorderColor = c);
            AddColorRow(windows, page, "Background Color (Active)", s => s.ActiveWindowBackgroundColor, (s, c) => s.ActiveWindowBackgroundColor = c);
        }

        private void BuildMiscPage(TabPage page)
        {
            TableLayoutPanel performance = BeginGroup(page, "Performance");

            _primaryDelayBox = AddTextRow(
                performance,
                page,
                "Primary Update Delay (ms)",
                "Delay for the main refresh cycle. Affects how often the currently opened windows (on all virtual desktops) are checked for changes.\n\nLower values update the pager faster, but can increase CPU usage.",
                s => s.PrimaryUpdateDelay,
                (s, v) => s.PrimaryUpdateDelay = v);

            _secondaryDelayBox = AddTextRow(
                performance,
                page,
                "Secondary Update Delay (ms)",
                "Delay for background/secondary updates. Affects how often the app window is checked for being covered by another one, this includes the Windows Taskbar. Also (re-)pins the app to all virtual desktops.\n\nSome of those operations require more processing, that's why this delay should be higher than the primary one.",
                s => s.SecondaryUpdateDelay,
                (s, v) => s.SecondaryUpdateDelay = v);

            TableLayoutPanel pinning = BeginGroup(page, "App Pinning");

            AddCheckBoxRow(
                pinning,
                page,
                "Show App In Taskbar",
                null,
                s => s.ShowAppInTaskbar,
                (s, v) => s.ShowAppInTaskbar = v);

            var note = new Label
            {
                Text = "Important: When deactivated, the automatic pinning of the app (to be visible on all virtual desktops) can't work anymore because of a limitation of the Windows OS. So the pinning has to be done manually after every application start using the \"Active Applications\" screen (Win + Tab).",
                AutoSize = true,
                MaximumSize = new Size(420, 0),
                ForeColor = SystemColors.GrayText,
                Margin = new Padding(8, 4, 8, 4),
            };

            var noteBox = new Panel
            {
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                BorderStyle = BorderStyle.FixedSingle,
                Padding = new Padding(8),
                Margin = new Padding(6, 12, 6, 4),
            };
            noteBox.Controls.Add(note);

            AddSpanningControl(pinning, noteBox);
        }

        // ------------------------------------------------------------------------------------
        // Placement helpers
        // ------------------------------------------------------------------------------------

        private Point FindPlacement(Rectangle owner, Size size)
        {
            return TryPlaceRight(owner, size)
                ?? TryPlaceLeft(owner, size)
                ?? TryPlaceBelow(owner, size)
                ?? TryPlaceAbove(owner, size)
                ?? CenterOverOwner(owner, size);
        }

        private Point? TryPlaceRight(Rectangle owner, Size size)
        {
            int x = owner.Right + PlacementGap;
            foreach (Screen screen in Screen.AllScreens)
            {
                Rectangle area = screen.WorkingArea;
                if (x >= area.Left && x + size.Width <= area.Right && size.Height <= area.Height)
                {
                    int y = Math.Max(area.Top, Math.Min(owner.Top, area.Bottom - size.Height));
                    return new Point(x, y);
                }
            }

            return null;
        }

        private Point? TryPlaceLeft(Rectangle owner, Size size)
        {
            int x = owner.Left - PlacementGap - size.Width;
            foreach (Screen screen in Screen.AllScreens)
            {
                Rectangle area = screen.WorkingArea;
                if (x >= area.Left && x + size.Width <= area.Right && size.Height <= area.Height)
                {
                    int y = Math.Max(area.Top, Math.Min(owner.Top, area.Bottom - size.Height));
                    return new Point(x, y);
                }
            }

            return null;
        }

        private Point? TryPlaceBelow(Rectangle owner, Size size)
        {
            int y = owner.Bottom + PlacementGap;
            foreach (Screen screen in Screen.AllScreens)
            {
                Rectangle area = screen.WorkingArea;
                if (y >= area.Top && y + size.Height <= area.Bottom && size.Width <= area.Width)
                {
                    int x = Math.Max(area.Left, Math.Min(owner.Left, area.Right - size.Width));
                    return new Point(x, y);
                }
            }

            return null;
        }

        private Point? TryPlaceAbove(Rectangle owner, Size size)
        {
            int y = owner.Top - PlacementGap - size.Height;
            foreach (Screen screen in Screen.AllScreens)
            {
                Rectangle area = screen.WorkingArea;
                if (y >= area.Top && y + size.Height <= area.Bottom && size.Width <= area.Width)
                {
                    int x = Math.Max(area.Left, Math.Min(owner.Left, area.Right - size.Width));
                    return new Point(x, y);
                }
            }

            return null;
        }

        private Point CenterOverOwner(Rectangle owner, Size size)
        {
            return new Point(
                owner.Left + ((owner.Width - size.Width) / 2),
                owner.Top + ((owner.Height - size.Height) / 2));
        }

        private sealed class RowBinding
        {
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
