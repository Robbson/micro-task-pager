using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Windows.Forms;
using MicroTaskPager.Config;
using MicroTaskPager.Core;
using MicroTaskPager.Core.API;
using MicroTaskPager.GUI.VirtualDesktop;
using Serilog;

namespace MicroTaskPager.GUI
{
    // Pager main application form
    public class MainForm : Form
    {
#pragma warning disable SA1401 // Fields should be private
        // Why should this be private? It is used in Virtual Desktop/Screens
        public ConcurrentBag<Window> Windows = new ConcurrentBag<Window>();
#pragma warning restore SA1401 // Fields should be private

        private static readonly ILogger _logger = Log.ForContext<MainForm>();

        // --- Internal Application State ---
        private readonly List<Desktop> _virtualDesktops = new List<Desktop>();
        private readonly ActiveWindowTracker _activeWindowTracker = new ActiveWindowTracker();
        private readonly HintForm _viewModeHint = new HintForm();
        private readonly HintForm _windowTitleHint = new HintForm();
        private readonly IntPtr _foregroundWindowAtStartup;
        private bool _isAppPinned = false;
        private int _activeDesktopIndex = 0;
        private int _currentDesktopCount = 0;
        private int _desktopSwitchVersion = 0;
        private string _screenConfigurationSignature = string.Empty;
        private string _windowsHash = string.Empty;
        private bool _isDialogOpen = false;
        private bool _isContextMenuOpen = false;
        private IntPtr _toolTipWindow = IntPtr.Zero;

        private Point dragOffset;

        // Manual window drag between desktops, with a common-controls drag image.
        private Window _dragCandidate;
        private int _dragOriginDesktopIndex = -1;
        private Rectangle _windowDragThreshold;
        private bool _windowDragActive;
        private ImageList _dragImageList;
        private Bitmap _dragIcon;

        public MainForm()
        {
            // Capture the window that was active before the pager shows up (the pager isn't shown yet)
            _foregroundWindowAtStartup = WinAPI.GetForegroundWindow();

            SuspendLayout();
            SetStyle(
                ControlStyles.ResizeRedraw
                | ControlStyles.OptimizedDoubleBuffer
                | ControlStyles.AllPaintingInWmPaint, true);

            // Form properties set to defaults
            Name = "frmMain";
            DoubleBuffered = true;

            // The pager is laid out in physical pixels and handles its own DPI scaling (see Dpi class),
            // so WinForms must not scale it a second time.
            AutoScaleMode = System.Windows.Forms.AutoScaleMode.None;
            StartPosition = FormStartPosition.Manual;
            ControlBox = false;
            MaximizeBox = false;
            MinimizeBox = false;
            TopMost = true;
            FormBorderStyle = System.Windows.Forms.FormBorderStyle.None;
            Icon = new System.Drawing.Icon(
                new MemoryStream(Helpers.GetResourceFromAssembly(typeof(Program), "MicroTaskPager.Resources.icon.ico")));

            var storedLocation = RegistryAccess.RestoreLocation();
            Location = storedLocation ?? GetDefaultLocation();

            Options = new AppSettings();
            Options = AppSettingsStore.Load();
            ApplySettings(Options, false);

            // Form properties set to configurable options
            BackColor = Options.DesktopBackgroundColor;
            Opacity = Options.BackgroundOpacity;
            ShowInTaskbar = Options.ShowAppInTaskbar;

            // Collect all virtual desktops and add mouse event listeners to them (uses PagerHeight)
            GetVirtualDesktopsAndAddMouseHandlers(_virtualDesktops);
            _screenConfigurationSignature = GetScreenConfigurationSignature();

            // Pager size depending on current amount of virtual desktops
            Size = new Size(_virtualDesktops.Sum(x => x.Size.Width), PagerHeightPixels);
            MinimumSize = Size;
            MaximumSize = Size;
            ClientSize = Size;

            ResumeLayout(false);

            // Event handler registration
            Shown += OnShown;
            MouseUp += OnMouseUp;
            MouseDown += OnMouseDown;
            MouseMove += OnMouseMove;
        }

        public bool IsDraggingWindow { get; set; }

        // True once a window tile press has moved past the drag threshold. Desktop mouse-up
        // handling checks this so a drop does not also switch or activate the desktop.
        public bool IsWindowDragActive => _windowDragActive;

        // Why property?
        public int BorderSize { get; } = Dpi.Scale(1);

        // PagerHeight is a logical value (device independent), all drawing uses physical pixels
        public int PagerHeightPixels => Dpi.Scale(Options.PagerHeight);

        // Application Options: Configurable by user
        public AppSettings Options { get; private set; }

        public IntPtr GetLastActiveWindow(int desktopIndex) => _activeWindowTracker.GetLastActiveWindow(desktopIndex);

        public void RememberActiveWindow(IntPtr hwnd) => _activeWindowTracker.RememberWindow(hwnd);

        // Restores the last active window of the desktop the pager just switched to
        public void RestoreLastActiveWindow(int desktopIndex)
        {
            _activeWindowTracker.RestoreLastActiveWindow(desktopIndex);

            // The desktop switch can still be in progress, so retry once after the switch animation
            int version = ++_desktopSwitchVersion;
            Task.Run(async () =>
            {
                await Task.Delay(150);
                TryInvoke(() =>
                {
                    if (version != _desktopSwitchVersion || GetActiveDesktopIndex() != desktopIndex)
                    {
                        return;
                    }

                    _activeWindowTracker.RestoreLastActiveWindow(desktopIndex);
                });
            });
        }

        // Activates a specific window of the desktop the pager just switched to
        public void ActivateWindowAfterDesktopSwitch(int desktopIndex, IntPtr hwnd)
        {
            ActivateWindow(hwnd);

            // The desktop switch can still be in progress, so retry once after the switch animation
            int version = ++_desktopSwitchVersion;
            Task.Run(async () =>
            {
                await Task.Delay(150);
                TryInvoke(() =>
                {
                    if (version != _desktopSwitchVersion || GetActiveDesktopIndex() != desktopIndex)
                    {
                        return;
                    }

                    ActivateWindow(hwnd);
                });
            });
        }

        // Arms a window drag. The drag image and threshold activation are handled here because
        // a drop can target any desktop, not only the one where the press happened.
        public void StartWindowDrag(Window window, int originDesktopIndex, Point start)
        {
            CancelWindowDrag();
            HideWindowToolTip();
            if (window == null)
            {
                return;
            }

            _dragCandidate = window;
            _dragOriginDesktopIndex = originDesktopIndex;

            Size dragSize = SystemInformation.DragSize;
            _windowDragThreshold = new Rectangle(
                new Point(start.X - (dragSize.Width / 2), start.Y - (dragSize.Height / 2)),
                dragSize);

            // Keep receiving mouse moves and the release if the cursor leaves the pager.
            Capture = true;
        }

        // Shows the current view mode briefly above (or below) the affected pager screen.
        public void ShowViewModeHint(ScreenRenderer renderer, ViewMode mode)
        {
            string modeName;
            switch (mode)
            {
                case ViewMode.Preview:
                    modeName = "Preview";
                    break;
                case ViewMode.MicroTaskbar:
                    modeName = "MicroTaskbar";
                    break;
                default:
                    modeName = "MicroTaskbar Light";
                    break;
            }

            Rectangle anchor = RectangleToScreen(new Rectangle(renderer.Location, renderer.Size));
            _viewModeHint.ShowAbove(modeName, anchor);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            try
            {
                _virtualDesktops.ForEach(x => x.Render(e));
            }
            catch
            {
                WindowsVirtualDesktop.Restart();
                WindowsVirtualDesktopManager.Restart();
            }
        }

        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            Invalidate();   // invalidates the whole client area at the new size
        }

        protected override void OnFormClosing(FormClosingEventArgs e)
        {
            // Stop the background polling loops before the window handle gets destroyed
            CancelWindowDrag();
            // Only hide the hint windows here. Disposing a shown form removes it from
            // Application.OpenForms while Application.Exit is enumerating that collection,
            // which throws "Collection was modified". The hints are closed with the other forms.
            _viewModeHint.HideHint();
            _windowTitleHint.HideHint();
            _activeWindowTracker.Dispose();
            Program.ApplicationClosing.Cancel();
            base.OnFormClosing(e);
        }

        private static void ActivateWindow(IntPtr hwnd)
        {
            if (hwnd == IntPtr.Zero || !WinAPI.IsWindow(hwnd) || !WinAPI.IsWindowVisible(hwnd))
            {
                return;
            }

            if (WinAPI.IsIconic(hwnd))
            {
                WinAPI.ShowWindow(hwnd, WinAPI.SW_RESTORE);
            }

            if (!WinAPI.SetForegroundWindow(hwnd))
            {
                _logger.Debug("Could not activate the window {Handle}", hwnd);
            }
        }

        // Detects monitors being added/removed or their resolution/arrangement changing while the app runs
        private static string GetScreenConfigurationSignature()
        {
            return string.Join(
                "|",
                Screen.AllScreens
                    .OrderBy(x => x.DeviceName, StringComparer.Ordinal)
                    .Select(x =>
                        $"{x.DeviceName}:" +
                        $"{x.Bounds.X},{x.Bounds.Y},{x.Bounds.Width},{x.Bounds.Height}:" +
                        $"{x.Primary}"));
        }

        private static int GetActiveDesktopIndex()
        {
            try
            {
                return WindowsVirtualDesktopManager.GetInstance()
                    .FromDesktop(WindowsVirtualDesktop.GetInstance().Current);
            }
            catch (Exception ex)
            {
                _logger.Debug(ex, "Could not determine the active desktop");
                return -1;
            }
        }

        private static Bitmap CreateTranslucentIcon(Bitmap icon, int size)
        {
            var result = new Bitmap(size, size, PixelFormat.Format32bppArgb);
            using (var graphics = Graphics.FromImage(result))
            using (var attributes = new ImageAttributes())
            {
                graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
                graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;

                // 75% opacity keeps the ghost readable without looking like a solid icon.
                var matrix = new ColorMatrix { Matrix33 = 0.85f };
                attributes.SetColorMatrix(matrix, ColorMatrixFlag.Default, ColorAdjustType.Bitmap);
                graphics.DrawImage(
                    icon,
                    new Rectangle(0, 0, size, size),
                    0,
                    0,
                    icon.Width,
                    icon.Height,
                    GraphicsUnit.Pixel,
                    attributes);
            }

            return result;
        }

        // Shows the window title next to the cursor while Ctrl is held over a pager tile.
        private void RefreshWindowToolTip()
        {
            if (IsDraggingWindow
                || _windowDragActive
                || _dragCandidate != null
                || _isDialogOpen
                || _isContextMenuOpen
                || (Control.ModifierKeys & Keys.Control) == Keys.None)
            {
                HideWindowToolTip();
                return;
            }

            Point clientPoint = PointToClient(Cursor.Position);
            Desktop desktop = _virtualDesktops.FirstOrDefault(d =>
                clientPoint.X >= d.Location.X && clientPoint.X < d.Location.X + d.Size.Width
                && clientPoint.Y >= d.Location.Y && clientPoint.Y < d.Location.Y + d.Size.Height);
            Window window = desktop?.GetWindowAt(clientPoint);
            if (window == null || string.IsNullOrEmpty(window.Title))
            {
                HideWindowToolTip();
                return;
            }

            if (window.Handle == _toolTipWindow)
            {
                return;
            }

            HideWindowToolTip();
            _toolTipWindow = window.Handle;

            _windowTitleHint.ShowNearCursor(window.Title, Cursor.Position);
        }

        private void HideWindowToolTip()
        {
            if (_toolTipWindow == IntPtr.Zero)
            {
                return;
            }

            _toolTipWindow = IntPtr.Zero;
            _windowTitleHint.HideHint();
        }

        private void UpdateWindowDrag(Point point, bool leftDown)
        {
            if (_dragCandidate == null)
            {
                return;
            }

            if (!_windowDragActive)
            {
                if (!leftDown)
                {
                    CancelWindowDrag();
                    return;
                }

                if (_windowDragThreshold.Contains(point))
                {
                    return;
                }

                _windowDragActive = true;
                StartWindowDragImage(_dragCandidate, point);
            }

            if (_dragImageList != null)
            {
                WinAPI.ImageList_DragMove(point.X, point.Y);
            }
        }

        private void CompleteWindowDrag(Point point)
        {
            Window dragged = _dragCandidate;
            int originDesktopIndex = _dragOriginDesktopIndex;
            bool wasActive = _windowDragActive;

            EndWindowDragImage();
            _dragCandidate = null;
            _windowDragActive = false;
            _dragOriginDesktopIndex = -1;
            if (Capture)
            {
                Capture = false;
            }

            if (!wasActive || dragged == null)
            {
                return;
            }

            Desktop target = _virtualDesktops.FirstOrDefault(d =>
                point.X >= d.Location.X && point.X < d.Location.X + d.Size.Width
                && point.Y >= d.Location.Y && point.Y < d.Location.Y + d.Size.Height);
            if (target == null || target.VirtualDesktopIndex == originDesktopIndex)
            {
                return;
            }

            try
            {
                WindowsVirtualDesktop.GetInstance().FromIndex(target.VirtualDesktopIndex).MoveWindow(dragged.Handle);
                Invalidate();
            }
            catch (Exception ex)
            {
                _logger.Error(
                    ex,
                    "Error while moving window {Handle} to desktop {DesktopIndex}",
                    dragged.Handle,
                    target.VirtualDesktopIndex);
            }
        }

        private void CancelWindowDrag()
        {
            EndWindowDragImage();
            _dragCandidate = null;
            _windowDragActive = false;
            _dragOriginDesktopIndex = -1;
            if (Capture)
            {
                Capture = false;
            }
        }

        private void StartWindowDragImage(Window window, Point point)
        {
            Bitmap icon = window?.Icon;
            if (icon == null)
            {
                return;
            }

            // Match the icon size used by the pager itself, so large application icons
            // (for example Chrome's) don't produce an oversized drag ghost.
            int iconSize = Math.Max(1, Dpi.Scale(Options.IconSize));
            var imageSize = new Size(iconSize, iconSize);
            var imageList = new ImageList
            {
                ColorDepth = ColorDepth.Depth32Bit,
                ImageSize = imageSize,
            };

            _dragIcon = CreateTranslucentIcon(icon, iconSize);
            imageList.Images.Add(_dragIcon);

            if (!WinAPI.ImageList_BeginDrag(imageList.Handle, 0, imageSize.Width / 2, imageSize.Height / 2))
            {
                imageList.Dispose();
                return;
            }

            if (!WinAPI.ImageList_DragEnter(Handle, point.X, point.Y))
            {
                WinAPI.ImageList_EndDrag();
                imageList.Dispose();
                return;
            }

            _dragImageList = imageList;
        }

        private void EndWindowDragImage()
        {
            if (_dragImageList != null)
            {
                WinAPI.ImageList_DragLeave(Handle);
                WinAPI.ImageList_EndDrag();
                _dragImageList.Dispose();
                _dragImageList = null;
            }

            _dragIcon?.Dispose();
            _dragIcon = null;
        }

        // Safe Invoke for the background loops: during shutdown the handle can disappear between
        // the loop check and the Invoke call, which throws InvalidOperationException
        private bool TryInvoke(Action action)
        {
            if (IsDisposed || !IsHandleCreated)
            {
                return false;
            }

            try
            {
                Invoke(action);
                return true;
            }
            catch (InvalidOperationException) when (IsDisposed || !IsHandleCreated)
            {
                return false;
            }
            catch (ObjectDisposedException)
            {
                return false;
            }
        }

        // Default app window location: Centered and above Taskbar
        private Point GetDefaultLocation() => new Point(
                (Screen.PrimaryScreen.Bounds.Width / 2) - (Size.Width / 2),
                Screen.PrimaryScreen.WorkingArea.Bottom - Size.Height);

        // App window covering detection: checks the center point, matching the Rust watchdog.
        // Partial overlaps that leave the center visible are recovered by the topmost reassert
        // that runs on every secondary tick.
        private bool IsCovered()
        {
            var rect = this.Bounds;
            var point = new WinAPI.POINT
            {
                X = rect.Left + rect.Width / 2,
                Y = rect.Top + rect.Height / 2,
            };
            var hwndAtPoint = WinAPI.WindowFromPoint(point);
            return hwndAtPoint != this.Handle;
        }

        private bool HasDesktopCountChanged() => _currentDesktopCount != WindowsVirtualDesktop.GetInstance().Count;

        private bool HasScreenConfigurationChanged()
        {
            var signature = GetScreenConfigurationSignature();
            if (signature == _screenConfigurationSignature)
            {
                return false;
            }

            _screenConfigurationSignature = signature;
            return true;
        }

        private void ResetVirtualDesktops()
        {
            // A drag references the old desktop list, so it cannot survive a reset.
            CancelWindowDrag();

            // Remove mouse listeners for all existing desktops first before removing them
            _virtualDesktops.ForEach((desktop) =>
            {
                MouseUp -= desktop.OnMouseUp;
                MouseDown -= desktop.OnMouseDown;
                MouseWheel -= desktop.OnMouseWheel;
            });

            WindowsVirtualDesktop.Restart();
            WindowsVirtualDesktopManager.Restart();

            _virtualDesktops.Clear();
            GetVirtualDesktopsAndAddMouseHandlers(_virtualDesktops);
            _screenConfigurationSignature = GetScreenConfigurationSignature();

            var newSize = new Size(_virtualDesktops.Sum(d => d.Size.Width), PagerHeightPixels);

            SuspendLayout();
            try
            {
                MaximumSize = Size.Empty;   // remove old cap first
                MinimumSize = Size.Empty;   // remove old floor first
                ClientSize = newSize;
                MinimumSize = newSize;
                MaximumSize = newSize;
            }
            finally
            {
                ResumeLayout();
            }
        }

        private void GetVirtualDesktopsAndAddMouseHandlers(List<Desktop> virtualDesktops)
        {
            _currentDesktopCount = WindowsVirtualDesktop.GetInstance().Count;
            Enumerable.Range(0, _currentDesktopCount).ToList().ForEach(d =>
            {
                Desktop desktop = new Desktop(d, this, new Point(_virtualDesktops.Sum(i => i.Size.Width), 0));
                MouseUp += desktop.OnMouseUp;
                MouseDown += desktop.OnMouseDown;
                MouseWheel += desktop.OnMouseWheel;
                virtualDesktops.Add(desktop);
            });
        }

        private void OnShown(object sender, EventArgs e)
        {
            // Remember the window that was active before the pager got shown
            _activeWindowTracker.Start(Handle);
            _activeWindowTracker.RememberWindow(_foregroundWindowAtStartup);

            // Primary application loop to check global window changes so the miniatures get updated
            Task.Run(async () =>
            {
                while (!Program.ApplicationClosing.IsCancellationRequested)
                {
                    if (!TryInvoke(() =>
                    {
                        try
                        {
                            // Change Detection via Hash (calculated on basic parameters of all opened windows) based on all windows sorted by their handles
                            Windows = new ConcurrentBag<Window>(WindowManager.GetOpenWindows(Options.IconSize <= 16));
                            var hash =
                            $"{_activeDesktopIndex}" +
                            $"{Windows.Sum(x => Math.Abs(x.Dimensions.X))}" +
                            $"{Windows.Sum(x => Math.Abs(x.Dimensions.Y))}" +
                            $"{Windows.Sum(x => x.Dimensions.Width)}" +
                            $"{Windows.Sum(x => x.Dimensions.Height)}" +
                            $"{string.Join(string.Empty, Windows.Select(x => x.IsActive ? 1 : 0))}" +
                            $"{string.Join(string.Empty, Windows.Select(x => WinAPI.GetForegroundWindow() == x.Handle ? 1 : 0))}" +
                            $"{string.Join(string.Empty, Windows.Select(x => x.VirtualDesktopIndex))}" +
                            $"{string.Join(",", Windows.Select(x => x.ZOrder))}" +
                            $"{WindowManager.IconRevision}";

                            if (hash != _windowsHash)
                            {
                                _windowsHash = hash;
                                Invalidate();
                            }
                        }
                        catch (Exception ex)
                        {
                            _logger.Error(ex, "Error while building change detection hash");
                        }

                        // Polling also catches Shift being pressed or released without mouse movement.
                        RefreshWindowToolTip();
                    }))
                    {
                        break;
                    }

                    // So we don't waste too many CPU cycles
                    await Task.Delay(Options.PrimaryUpdateDelay);
                }
            });

            // Secondary application loop for pinning the app, bringing it to front when hidden by some other window
            // and detect desktop count changes
            // -> only pinned apps are visible on all virtual desktops, which is a requirement for this app
            Task.Run(async () =>
            {
                while (!Program.ApplicationClosing.IsCancellationRequested)
                {
                    if (!TryInvoke(() =>
                    {
                        // Reassert topmost on every tick, like the Rust implementations. This also
                        // recovers partial overlaps that do not cover the center of the pager.
                        // Skip while one of our own dialogs, the context menu, or a hint is open
                        // so the pager cannot cover them.
                        if (!_isDialogOpen
                            && !_isContextMenuOpen
                            && !_viewModeHint.Visible
                            && !_windowTitleHint.Visible)
                        {
                            // Check if window is covered should be fine with every 500ms
                            if (IsCovered())
                            {
                                WindowManager.RestoreWindow(Handle);
                            }

                            WindowManager.ReassertTopmost(Handle);
                        }

                        if (HasDesktopCountChanged() || HasScreenConfigurationChanged())
                        {
                            ResetVirtualDesktops();
                        }

                        // This works only when the pinning is manually done by the user after application has started
                        // (because auto pinning will always fail)

                        // This should only be the case at start up, right
                        if (Options.ShowAppInTaskbar && !_isAppPinned)
                        {
                            try
                            {
                                // Note: The internal Divided by Zero error happens here
                                WindowsVirtualDesktopManager.GetInstance().PinApplication(Handle);
                                _isAppPinned = true;
                            }
                            catch (Exception ex)
                            {
                                _logger.Error(ex, "Error while pinning application " + Handle);
                            }
                        }

                        try
                        {
                            _activeDesktopIndex = WindowsVirtualDesktopManager.GetInstance()
                                .FromDesktop(WindowsVirtualDesktop.GetInstance().Current);
                            Windows = new ConcurrentBag<Window>(WindowManager.GetOpenWindows(Options.IconSize <= 16));
                            //Invalidate();
                        }
                        catch (Exception ex)
                        {
                            _logger.Error(ex, "Error while trying to get Active Desktop Index");
                        }
                    }))
                    {
                        break;
                    }

                    await Task.Delay(Options.SecondaryUpdateDelay);
                }
            });
        }

        private void OnMouseUp(object sender, MouseEventArgs e)
        {
            bool wasDraggingWindow = IsDraggingWindow;
            IsDraggingWindow = false;
            Cursor = Cursors.Default;

            CompleteWindowDrag(e.Location);

            if ((e.Button & MouseButtons.Right) == MouseButtons.Right)
            {
                ShowContextMenu(e.X, e.Y);
                return;
            }

            if (wasDraggingWindow)
            {
                // Dragging the pager activated it. Return the foreground to the last active
                // application window of the current desktop, like a desktop selection does.
                int desktopIndex = GetActiveDesktopIndex();
                if (desktopIndex >= 0)
                {
                    RestoreLastActiveWindow(desktopIndex);
                }
            }
        }

        private void OnMouseDown(object sender, MouseEventArgs e)
        {
            if ((e.Button & MouseButtons.Middle) == MouseButtons.Middle || (Control.ModifierKeys & Keys.Control) != Keys.None)
            {
                IsDraggingWindow = true;
                dragOffset = e.Location;
                Cursor = Cursors.SizeAll;
                HideWindowToolTip();
            }
        }

        private void OnMouseMove(object sender, MouseEventArgs e)
        {
            RefreshWindowToolTip();

            if (IsDraggingWindow)
            {
                Location = new Point(e.X + Location.X - dragOffset.X, e.Y + Location.Y - dragOffset.Y);
                return;
            }

            UpdateWindowDrag(e.Location, (e.Button & MouseButtons.Left) == MouseButtons.Left);
        }

        private void ShowContextMenu(int mouseX, int mouseY)
        {
            //_forceAppAlwaysOnTop = false;
            ContextMenuStrip menu = new ContextMenuStrip();
            menu.ImageScalingSize = new Size(Dpi.Scale(16), Dpi.Scale(16));

            Helpers.AddMenuItem(this, menu,
                 new ToolStripMenuItem()
                 {
                     Text = "Toggle View Mode",
                     Image = Helpers.CreateGlyphBitmap("↔"),
                 },
                 () =>
                 {
                     // Change a desktop's mode using a faked mouse wheel event including a hacky impossible wheel-direction: none
                     MouseEventArgs eventArgs = new MouseEventArgs(MouseButtons.None, 0, mouseX, mouseY, 0);
                     this.OnMouseWheel(eventArgs);
                 });

            // --- Position related ---
            ToolStripMenuItem positionMenu = new ToolStripMenuItem()
            {
                Text = "Position",
                Image = Helpers.CreateGlyphBitmap("⌖"),
            };

            ToolStripMenuItem buttonRestorePos = new ToolStripMenuItem("Restore", null, (s, ev) =>
            {
                var storedLocation = RegistryAccess.RestoreLocation();
                if (storedLocation != null) Location = storedLocation.Value;
            });

            ToolStripMenuItem buttonSavePos = new ToolStripMenuItem("Save", null, (s, ev) => RegistryAccess.SaveLocation(Location));
            ToolStripMenuItem buttonDefaultPos = new ToolStripMenuItem("Default", null, (s, ev) => Location = GetDefaultLocation());

            positionMenu.DropDownItems.AddRange(new ToolStripItem[] { buttonRestorePos, buttonSavePos, buttonDefaultPos });
            menu.Items.Add(positionMenu);

            // --- Aditional menu entries ---
            Helpers.AddMenuItem(this, menu,
                new ToolStripMenuItem()
                {
                    Text = "Settings",
                    Image = Helpers.CreateGlyphBitmap("⚙"),
                },
                () => OpenSettingsDialog());

            Helpers.AddMenuItem(this, menu,
                new ToolStripMenuItem()
                {
                    Text = "About",
                    Image = Helpers.CreateGlyphBitmap("ℹ"),
                },
                () => OpenAboutDialog());

            Helpers.AddMenuItem(this, menu,
                new ToolStripMenuItem()
                {
                    Text = "Exit",
                    Image = Helpers.CreateGlyphBitmap("✕"),
                },
                () => Application.Exit());

            //menu.Opened += (ss, ee) => _forceAppAlwaysOnTop = false;
            menu.Opened += (s, e) => _isContextMenuOpen = true;
            menu.Closed += (s, e) => _isContextMenuOpen = false;
            menu.Show(this, PointToClient(Cursor.Position));
        }

        private void OpenSettingsDialog()
        {
            var originalSettings = AppSettings.Clone(Options);
            var settingsWindow = new SettingsDialog(Options, Bounds);

            settingsWindow.PreviewChanged += s => PreviewSettings(s);

            _isDialogOpen = true;
            try
            {
                DialogResult result = settingsWindow.ShowDialog(this);
                if (result == DialogResult.OK)
                {
                    ApplySettings(settingsWindow.Settings, true); // save and reset to new settings
                }
                else
                {
                    // Don't assign Options beforehand: ApplySettings compares against it to detect layout changes
                    ApplySettings(originalSettings, false); // restore visuals
                }
            }
            finally
            {
                _isDialogOpen = false;
            }
        }

        private void PreviewSettings(AppSettings settings) => ApplySettings(settings, false);

        private void ApplySettings(AppSettings settings, bool persist)
        {
            bool resetDesktops = false;
            if (settings.PagerHeight != Options.PagerHeight) resetDesktops = true;

            // The small and large icon variants come from different sources. Refreshing the
            // snapshot before painting makes the live preview match the result after saving.
            bool reloadIcons = (settings.IconSize <= 16) != (Options.IconSize <= 16);

            Options = settings;

            // Apply MainForm propertes
            ShowInTaskbar = Options.ShowAppInTaskbar;
            BackColor = Options.DesktopBackgroundColor;
            Opacity = Math.Max(0.25, Math.Min(1.0, Options.BackgroundOpacity));

            if (reloadIcons)
            {
                Windows = new ConcurrentBag<Window>(WindowManager.GetOpenWindows(Options.IconSize <= 16));
            }

            if (persist)
            {
                AppSettingsStore.Save(settings);
            }

            if (persist || resetDesktops)
            {
                ResetVirtualDesktops();
            }

            Invalidate();
        }

        private void OpenAboutDialog()
        {
            using (var aboutDialog = new AboutDialog())
            {
                _isDialogOpen = true;
                try
                {
                    aboutDialog.ShowDialog(this);
                }
                finally
                {
                    _isDialogOpen = false;
                }
            }
        }
    }
}
