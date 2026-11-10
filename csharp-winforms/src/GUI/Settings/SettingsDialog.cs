using System;
using System.Collections.Generic;
using System.Drawing;
using System.Globalization;
using System.Windows.Forms;
using MicroTaskPager.Config;

namespace MicroTaskPager.GUI
{
    // Application side of the settings dialog: the concrete tabs and options, validation and dialog
    // placement. The declarative row/tab framework it builds on lives in SettingsWindow.Framework.cs.
    public partial class SettingsDialog : Form
    {
        private const int PlacementGap = 8;
        private const int MinPagerContentHeight = 16;

        private static readonly int[] _iconSizes = { 16, 24, 32, 48, 64 };

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

        public SettingsDialog(AppSettings settings, Rectangle ownerBounds)
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

        // WS_EX_COMPOSITED makes Windows paint the dialog bottom-up, which removes the flicker when
        // TabControl swaps the visible page in and out.
        protected override CreateParams CreateParams
        {
            get
            {
                CreateParams cp = base.CreateParams;
                cp.ExStyle |= 0x02000000;
                return cp;
            }
        }

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
        // Static helpers
        // ------------------------------------------------------------------------------------

        private static int IconSizeIndex(int size)
        {
            int bestIndex = 0;
            for (int i = 1; i < _iconSizes.Length; i++)
            {
                if (Math.Abs(_iconSizes[i] - size) < Math.Abs(_iconSizes[bestIndex] - size))
                {
                    bestIndex = i;
                }
            }

            return bestIndex;
        }

        // The combo box lists "No" last even though it is the default and has the value 0.
        private static int MinimizedVisibilityIndex(MinimizedWindowVisibility visibility)
        {
            switch (visibility)
            {
                case MinimizedWindowVisibility.Yes:
                    return 0;
                case MinimizedWindowVisibility.AfterOthers:
                    return 1;
                default:
                    return 2;
            }
        }

        private static MinimizedWindowVisibility MinimizedVisibilityFromIndex(int index)
        {
            switch (index)
            {
                case 0:
                    return MinimizedWindowVisibility.Yes;
                case 1:
                    return MinimizedWindowVisibility.AfterOthers;
                default:
                    return MinimizedWindowVisibility.No;
            }
        }

        // ------------------------------------------------------------------------------------
        // UI construction
        // ------------------------------------------------------------------------------------

        private void InitializeComponent()
        {
            SuspendLayout();

            // Composite the dialog and its children so switching tabs does not show the page being
            // laid out/painted. The pages themselves are built once in this method.
            DoubleBuffered = true;

            // Pixel layout is authored at 96 DPI; let WinForms scale it like AboutDialog does.
            AutoScaleDimensions = new SizeF(96F, 96F);
            AutoScaleMode = AutoScaleMode.Dpi;

            Text = "Micro Task Pager Settings";
            ClientSize = new Size(520, 570);
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

            // ---- Add tabs and their pages ----
            _tabs = new TabControl
            {
                Dock = DockStyle.Fill,
            };

            _tabs.TabPages.Add(CreateLayoutTab());
            _tabs.TabPages.Add(CreateThemeTab());
            _tabs.TabPages.Add(CreateMiscTab());

            // ---- Add generic dialog buttons at bottom ----
            var buttons = new FlowLayoutPanel
            {
                Dock = DockStyle.Bottom,
                FlowDirection = FlowDirection.RightToLeft,
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                WrapContents = false,
                Padding = new Padding(0, 4, 0, 4),
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

        private TabPage CreateLayoutTab()
        {
            TabPage layoutPage = CreateTabPage("Layout");

            Render(
                BeginGroup(layoutPage, "Rendering"),
                ComboRow(
                    "View Mode Default",
                    new[] { "Preview", "MicroTaskbar" },
                    s => (int)s.ViewModeDefault,
                    (s, v) => s.ViewModeDefault = (ViewMode)v,
                    toolTip: "Only affects newly created desktops and their screens."),
                NumericRow(
                    "Pager Height",
                    MinPagerContentHeight,
                    9999,
                    s => s.PagerHeight,
                    (s, v) => s.PagerHeight = v,
                    toolTip: "The pager's width is automatically calculated from this value and from the size of all available screen space as well as the amount of virtual desktops.",
                    normalize: value => Math.Max(value, _working.IconPaddingY + MinPagerContentHeight)));

            // Snap the working icon size to the nearest entry the slider can represent.
            _working.IconSize = _iconSizes[IconSizeIndex(_working.IconSize)];

            Render(
                BeginGroup(layoutPage, "Icons Settings"),
                SliderRow(
                    "Icon Size",
                    0,
                    _iconSizes.Length - 1,
                    s => IconSizeIndex(s.IconSize),
                    (s, p) => s.IconSize = _iconSizes[p],
                    p => $"{_iconSizes[p]} px",
                    p => _iconSizes[p] != _settingsBefore.IconSize,
                    toolTip: "Size of an icon in both Preview and Micro Taskbar view modes. Icons are drawn as squares, so a single value is enough.\n\n16 uses the small window icon. Larger values use the window's large icon, so the shown logo matches the one in the Windows taskbar."));

            Render(
                BeginGroup(layoutPage, "MicroTaskbar"),
                NumericRow(
                    "Icon Padding X",
                    -9999,
                    9999,
                    s => s.IconPaddingX,
                    (s, v) => s.IconPaddingX = v,
                    toolTip: "Negative values are supported as well so less space is taken and the icons look stacked."),
                NumericRow(
                    "Icon Padding Y",
                    -9999,
                    9999,
                    s => s.IconPaddingY,
                    (s, v) => s.IconPaddingY = v,
                    toolTip: "Negative values are supported as well in case of a very small pager height, like 40px which is usually the size of the Windows Taskbar."),
                ComboRow(
                    "Show When Minimized",
                    new[] { "Yes", "Yes, after the others", "No" },
                    s => MinimizedVisibilityIndex(s.ShowWhenMinimized),
                    (s, v) => s.ShowWhenMinimized = MinimizedVisibilityFromIndex(v),
                    toolTip: "Defines whether minimized windows are shown in the Micro Taskbar view mode.\n\nYes: Minimized windows are shown in their normal order.\n\nYes, after the others: Minimized windows are shown after all normal windows.\n\nNo: Minimized windows are hidden.\n\nMinimized windows are painted darker to tell them apart from normal windows."));

            return layoutPage;
        }

        private TabPage CreateThemeTab()
        {
            TabPage themePage = CreateTabPage("Theme");

            Render(
                BeginGroup(themePage, "General"),
                SliderRow(
                    "App Background Opacity",
                    25,
                    100,
                    s => (int)Math.Round(s.BackgroundOpacity * 100),
                    (s, p) => s.BackgroundOpacity = p / 100.0,
                    p => FormatOpacity(p / 100.0),
                    p => Math.Abs((p / 100.0) - _settingsBefore.BackgroundOpacity) > 0.0001,
                    tickFrequency: 5,
                    largeChange: 10));

            Render(
                BeginGroup(themePage, "Desktops"),
                ComboRow(
                    "Border Style",
                    new[] { "Box", "Underline" },
                    s => (int)s.DesktopBorderStyle,
                    (s, v) => s.DesktopBorderStyle = (Config.DesktopBorderStyle)v),
                ColorRow("Background Color", s => s.DesktopBackgroundColor,
                    (s, c) => s.DesktopBackgroundColor = c),
                ColorRow("Border Color", s => s.DesktopBorderColor,
                    (s, c) => s.DesktopBorderColor = c),
                ColorRow("Border Color (Active)", s => s.ActiveDesktopBorderColor,
                    (s, c) => s.ActiveDesktopBorderColor = c));

            Render(
                BeginGroup(themePage, "Preview Windows"),
                ColorRow("Background Color", s => s.WindowBackgroundColor,
                    (s, c) => s.WindowBackgroundColor = c),
                ColorRow("Border Color", s => s.WindowBorderColor,
                    (s, c) => s.WindowBorderColor = c),
                ColorRow("Border Color (Active)", s => s.ActiveWindowBorderColor,
                    (s, c) => s.ActiveWindowBorderColor = c),
                ColorRow("Background Color (Active)", s => s.ActiveWindowBackgroundColor,
                    (s, c) => s.ActiveWindowBackgroundColor = c));

            return themePage;
        }

        private TabPage CreateMiscTab()
        {
            TabPage miscPage = CreateTabPage("Misc");

            Render(
                BeginGroup(miscPage, "Behaviour"),
                ComboRow(
                    "Desktop Change: Window Activation",
                    new[] { "Desktop's last window", "Window under cursor", "None" },
                    s => (int)s.DesktopChangeActivation,
                    (s, v) => s.DesktopChangeActivation = (DesktopChangeActivationMode)v,
                    toolTip: "Defines which window gets activated when another desktop is selected in the pager.\n\nDesktop's last window: The last activated window on the selected desktop is reactivated automatically.\n\nWindow under cursor: The app (icon or window) that is under the mouse pointer when the desktop is selected is activated instead.\n\nNone: No window is activated, so the currently active window stays unchanged."));

            RowSpec primaryDelay = TextRow(
                "Primary Update Delay (ms)",
                s => s.PrimaryUpdateDelay,
                (s, v) => s.PrimaryUpdateDelay = v,
                toolTip: "Delay for the main refresh cycle. Affects how often the currently opened windows (on all virtual desktops) are checked for changes.\n\nLower values update the pager faster, but can increase CPU usage.");

            RowSpec secondaryDelay = TextRow(
                "Secondary Update Delay (ms)",
                s => s.SecondaryUpdateDelay,
                (s, v) => s.SecondaryUpdateDelay = v,
                toolTip: "Delay for background/secondary updates. Affects how often the app window is checked for being covered by another one, this includes the Windows Taskbar. Also (re-)pins the app to all virtual desktops.\n\nSome of those operations require more processing, that's why this delay should be higher than the primary one.");

            _primaryDelayBox = (TextBox)primaryDelay.Host;
            _secondaryDelayBox = (TextBox)secondaryDelay.Host;

            Render(BeginGroup(miscPage, "Performance"), primaryDelay, secondaryDelay);

            RowGroup pinningGroup = BeginGroup(miscPage, "App Pinning");

            Render(
                pinningGroup,
                CheckBoxRow(
                    "Show App In Taskbar",
                    s => s.ShowAppInTaskbar,
                    (s, v) => s.ShowAppInTaskbar = v));

            var noteBox = new Panel
            {
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                BorderStyle = BorderStyle.FixedSingle,
                Padding = new Padding(8),
                Margin = new Padding(6, 12, 6, 4),
            };

            var note = new Label
            {
                Text = "Important: When deactivated, the automatic pinning of the app (to be visible on all virtual desktops) can't work anymore because of a limitation of the Windows OS. So the pinning has to be done manually after every application start using the \"Active Applications\" screen (Win + Tab).",
                AutoSize = true,
                MaximumSize = new Size(420, 0),
                ForeColor = SystemColors.GrayText,
                Margin = new Padding(8, 4, 8, 4),
            };
            noteBox.Controls.Add(note);

            AddSpanningControl(pinningGroup.Table, noteBox);

            return miscPage;
        }

        // ------------------------------------------------------------------------------------
        // Commands
        // ------------------------------------------------------------------------------------

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
        // Validation
        // ------------------------------------------------------------------------------------

        private AppSettings TryBuildSettings(bool showValidationErrors)
        {
            if (!int.TryParse(
                    (string)_primaryDelayBox.Text,
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
                    (string)_secondaryDelayBox.Text,
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

        private void ShowValidationError(string message)
        {
            MessageBox.Show(this, message, "Invalid Settings", MessageBoxButtons.OK, MessageBoxIcon.Warning);
        }

        // ------------------------------------------------------------------------------------
        // Placement helpers
        // ------------------------------------------------------------------------------------

        /// <summary>
        /// Chooses a dialog location relative to the pager: right, left, below, above, then centered
        /// as a last resort. The owner bounds and dialog size are both required, and every candidate
        /// must fit fully inside one screen's working area.
        /// </summary>
        /// <returns>The preferred top-left position.</returns>
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
    }
}
