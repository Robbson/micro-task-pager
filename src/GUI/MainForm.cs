using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Windows.Forms;
using System.Windows.Interop;
using MicroTaskPager.Config;
using MicroTaskPager.Core;
using MicroTaskPager.Core.API;

namespace MicroTaskPager.GUI
{
    // Pager main application form
    public class MainForm : Form
    {
#pragma warning disable SA1401 // Fields should be private
        // Why should this be private? It is used in Virtual Desktop/Screens
        public ConcurrentBag<Window> Windows = new ConcurrentBag<Window>();
#pragma warning restore SA1401 // Fields should be private

        // --- Internal Application State ---
        private readonly List<VirtualDesktop> _virtualDesktops = new List<VirtualDesktop>();
        private bool _isAppPinned = false;
        private int _activeDesktopIndex = 0;
        private int _currentDesktopCount = 0;
        private string _windowsHash = string.Empty;

        private Point dragOffset;

        public MainForm()
        {
            SuspendLayout();
            SetStyle(
                ControlStyles.ResizeRedraw
                | ControlStyles.OptimizedDoubleBuffer
                | ControlStyles.AllPaintingInWmPaint, true);

            // Form properties set to defaults
            Name = "frmMain";
            DoubleBuffered = true;
            AutoScaleDimensions = new System.Drawing.SizeF(6F, 13F);
            AutoScaleMode = System.Windows.Forms.AutoScaleMode.Font;
            StartPosition = FormStartPosition.Manual;
            ControlBox = false;
            MaximizeBox = false;
            MinimizeBox = false;
            TopMost = true;
            AllowDrop = true;
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

            // Pager size depending on current amount of virtual desktops
            Size = new Size(_virtualDesktops.Sum(x => x.Size.Width), Options.PagerHeight);
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

        // Why property?
        public int BorderSize { get; } = 1;

        // Application Options: Configurable by user
        public AppSettings Options { get; private set; }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            try
            {
                _virtualDesktops.ForEach(x => x.OnPaint(e));
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
            Program.ApplicationClosing.Cancel();
            base.OnFormClosing(e);
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

        // App Window covering detection: Currently only by its center point
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

        private void ResetVirtualDesktops()
        {
            // Remove mouse listeners for all existing desktops first before removing them
            _virtualDesktops.ForEach((desktop) =>
            {
                MouseUp -= desktop.OnMouseUp;
                MouseDown -= desktop.OnMouseDown;
                MouseMove -= desktop.OnMouseMove;
                DragOver -= desktop.OnDragOver;
                DragDrop -= desktop.OnDragDrop;
            });

            WindowsVirtualDesktop.Restart();
            WindowsVirtualDesktopManager.Restart();

            _virtualDesktops.Clear();
            GetVirtualDesktopsAndAddMouseHandlers(_virtualDesktops);

            var newSize = new Size(_virtualDesktops.Sum(d => d.Size.Width), Options.PagerHeight);

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

        private void GetVirtualDesktopsAndAddMouseHandlers(List<VirtualDesktop> virtualDesktops)
        {
            _currentDesktopCount = WindowsVirtualDesktop.GetInstance().Count;
            Enumerable.Range(0, _currentDesktopCount).ToList().ForEach(d =>
            {
                VirtualDesktop desktop = new VirtualDesktop(d, this, new Point(_virtualDesktops.Sum(i => i.Size.Width), 0));
                MouseUp += desktop.OnMouseUp;
                MouseDown += desktop.OnMouseDown;
                MouseMove += desktop.OnMouseMove;
                DragOver += desktop.OnDragOver;
                DragDrop += desktop.OnDragDrop;
                virtualDesktops.Add(desktop);
            });
        }

        private void OnShown(object sender, EventArgs e)
        {
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
                            Windows = new ConcurrentBag<Window>(WindowManager.GetOpenWindows());
                            var hash =
                            $"{_activeDesktopIndex}" +
                            $"{Windows.Sum(x => Math.Abs(x.Dimensions.X))}" +
                            $"{Windows.Sum(x => Math.Abs(x.Dimensions.Y))}" +
                            $"{Windows.Sum(x => x.Dimensions.Width)}" +
                            $"{Windows.Sum(x => x.Dimensions.Height)}" +
                            $"{string.Join(string.Empty, Windows.Select(x => x.IsActive ? 1 : 0))}" +
                            $"{string.Join(string.Empty, Windows.Select(x => WinAPI.GetForegroundWindow() == x.Handle ? 1 : 0))}" +
                            $"{string.Join(string.Empty, Windows.Select(x => x.VirtualDesktopIndex))}";

                            if (hash != _windowsHash)
                            {
                                _windowsHash = hash;
                                Invalidate();
                            }
                        }
                        catch
                        {
                        }
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
                        // Check if window is covered should be fine with every 500ms
                        if (IsCovered())
                        {
                            WindowManager.RestoreWindow(Handle);
                        }

                        if (HasDesktopCountChanged())
                        {
                            // TODO: Adjust the list length for the individual view settings
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
                            catch
                            {
                            }
                        }

                        try
                        {
                            _activeDesktopIndex = WindowsVirtualDesktopManager.GetInstance()
                                .FromDesktop(WindowsVirtualDesktop.GetInstance().Current);
                            Windows = new ConcurrentBag<Window>(WindowManager.GetOpenWindows());
                            //Invalidate();
                        }
                        catch
                        {
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
            IsDraggingWindow = false;
            Cursor = Cursors.Default;
            if ((e.Button & MouseButtons.Right) == MouseButtons.Right)
            {
                ShowContextMenu();
            }
        }

        private void OnMouseDown(object sender, MouseEventArgs e)
        {
            if ((e.Button & MouseButtons.Middle) == MouseButtons.Middle)
            {
                IsDraggingWindow = true;
                dragOffset = e.Location;
                Cursor = Cursors.SizeAll;
            }
        }

        private void OnMouseMove(object sender, MouseEventArgs e)
        {
            if (IsDraggingWindow)
            {
                //Cursor = Cursors.SizeAll;
                Location = new Point(e.X + Location.X - dragOffset.X, e.Y + Location.Y - dragOffset.Y);
            }
        }

        private void ShowContextMenu()
        {
            //_forceAppAlwaysOnTop = false;
            ContextMenuStrip menu = new ContextMenuStrip();

            Helpers.AddMenuItem(this, menu,
                 new ToolStripMenuItem()
                 {
                     Text = "Toggle Desktop Mode",
                     Image = Helpers.CreateGlyphBitmap("↔"),
                 },
                 () =>
                 {
                     // TODO: Individual Mode per desktop
                     Options.DesktopModeDefault = Options.DesktopModeDefault == DesktopMode.MicroTaskbar ? DesktopMode.Preview : DesktopMode.MicroTaskbar;
                     AppSettingsStore.Save(Options);
                     Invalidate();
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
                () => Environment.Exit(0));

            //menu.Opened += (ss, ee) => _forceAppAlwaysOnTop = false;
            menu.Show(this, PointToClient(Cursor.Position));
        }

        private void OpenSettingsDialog()
        {
            var originalSettings = AppSettings.Clone(Options);
            var settingsWindow = new AppSettingsWindow(Options);
            new WindowInteropHelper(settingsWindow) { Owner = Handle };
            settingsWindow.PositionNextToOwner(Bounds);

            settingsWindow.PreviewChanged += s => PreviewSettings(s);

            bool? result = settingsWindow.ShowDialog();
            if (result == true)
            {
                ApplySettings(settingsWindow.Settings, true); // save and reset to new settings
            }
            else
            {
                // Don't assign Options beforehand: ApplySettings compares against it to detect layout changes
                ApplySettings(originalSettings, false); // restore visuals
            }
        }

        private void PreviewSettings(AppSettings settings) => ApplySettings(settings, false);

        private void ApplySettings(AppSettings settings, bool persist)
        {
            bool resetDesktops = false;
            if (settings.PagerHeight != Options.PagerHeight) resetDesktops = true;

            Options = settings;

            // Apply MainForm propertes
            ShowInTaskbar = Options.ShowAppInTaskbar;
            BackColor = Options.DesktopBackgroundColor;
            Opacity = Math.Max(0.25, Math.Min(1.0, Options.BackgroundOpacity));

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
                aboutDialog.ShowDialog(this);
            }
        }
    }
}
