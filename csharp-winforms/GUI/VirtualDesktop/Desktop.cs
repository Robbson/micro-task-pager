using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Linq;
using System.Threading.Tasks;
using System.Windows.Forms;
using MicroTaskPager.Config;
using MicroTaskPager.Core;
using MicroTaskPager.Core.API;
using DesktopBorderStyle = MicroTaskPager.Config.DesktopBorderStyle;

namespace MicroTaskPager.GUI.VirtualDesktop
{
    // Holds all data for a desktop shown as a rectangle in the app, manages a ScreenRenderer for each physical PC screen
    // - this Desktop class only handles selecting a desktop, drag & drop windows inbetween and painting the selection border,
    //   while the ScreenRenderer draws all the content
    // - It's no WinForm Control! It just receives direct rendering requests from the MainForm and also forces the rendering ob its Screen Renderer child classes
    public class Desktop
    {
        private readonly List<ScreenRenderer> _screenRenderers = new List<ScreenRenderer>();
        private readonly int _numScreens;
        private bool _pagerDragGesture;

        // Used only in MainFrame when ResetVirtualDesktops() ist called (for each desktop)
        public Desktop(int virtualDesktopIndex, MainForm form, Point location)
        {
            Form = form;
            Location = location;
            VirtualDesktopIndex = virtualDesktopIndex;

            _screenRenderers.Clear();

            // List of physicial screens from left to right and top to down
            // -> But we have no support for screens that are aligned vertically in Windows which leads to an interesting bug:
            //    When the window is moved DOWN to the other screen, the right part of the desktop can't show it because it's
            //    displaying an area of the screen which is not there
            // => TODO: The only natural solution would be a columns of desktops or a (partial) grid for both directions
            var physcialScreens = Screen.AllScreens.OrderBy(x => x.Bounds.Left).ThenBy(x => x.Bounds.Top);
            _numScreens = physcialScreens.Count();
            var screen_cnt = 0;
            foreach (var screen in physcialScreens)
            {
                var index = VirtualDesktopIndex + screen_cnt;
                var mode = Form.Options.ViewModeDefault;
                if (index < Form.Options.RendererViewModes.Count)
                {
                    mode = Form.Options.RendererViewModes[index];
                }
                else
                {
                    // Fill missing items using the default mode
                    Form.Options.RendererViewModes.Add(mode);
                }

                _screenRenderers.Add(new ScreenRenderer()
                {
                    Form = Form,
                    VirtualDesktop = this,
                    AttachedScreen = screen,
                    Location = new Point(location.X + _screenRenderers.Sum(x => x.Size.Width), location.Y),
                    Size = Helpers.AspectRatioResize(new Size(screen.Bounds.Width, screen.Bounds.Height), 0, Form.PagerHeightPixels),
                    Mode = mode,
                });
                screen_cnt++;
            }

            Size = new Size(_screenRenderers.Sum(x => x.Size.Width), Form.PagerHeightPixels);
        }

        public MainForm Form { get; set; }
        public Size Size { get; set; }
        public Point Location { get; set; }
        public int VirtualDesktopIndex { get; set; }

        public bool IsCurrentActiveDesktop
        {
            get => WindowsVirtualDesktopManager.GetInstance().FromDesktop(WindowsVirtualDesktop.GetInstance().Current) == VirtualDesktopIndex;
        }

        public void OnMouseDown(object sender, MouseEventArgs e)
        {
            // Ctrl+left starts a pager drag in MainForm instead of a desktop interaction.
            // Remember that at press time: the user may release Ctrl before the mouse button,
            // and the mouse-up must not select or activate this desktop in that case.
            _pagerDragGesture = (Control.ModifierKeys & Keys.Control) != Keys.None;

            if (_pagerDragGesture)
            {
                return;
            }

            if (!IsInsideBounds(e.X, e.Y)) return;
            if ((e.Button & MouseButtons.Left) == MouseButtons.Left)
            {
                var w = GetWindowAt(e.Location);
                if (w != null)
                {
                    // Bring a window into front by clicking on its miniature / icon
                    // -> only when it's already the active desktop, which makes desktop switches smoother
                    //    without touching the last window z-order
                    // -> Now this is done in MouseDown again so the app seems more responsive and more importantly,
                    //    the Pager window doesn't get activated too long (leads to nervousness in the Taskbar)
                    if (IsCurrentActiveDesktop)
                    {
                        // Shift-clicking a window tile in the full Micro Taskbar minimizes it.
                        // The light mode and preview don't show minimized windows, so minimizing
                        // is limited to the full mode. An already minimized window is never
                        // minimized again; clicking it restores it as usual.
                        bool minimizeWindow =
                            (Control.ModifierKeys & Keys.Shift) != Keys.None
                            && !w.IsMinimized
                            && IsMicroTaskbarAt(e.X);

                        if (minimizeWindow)
                        {
                            WinAPI.ShowWindow(w.Handle, WinAPI.SW_MINIMIZE);

                            // Never fall through here: the window is iconic now, so the restore
                            // branch below would bring it back immediately. Minimizing also must
                            // not focus the window or start a drag & drop operation.
                            return;
                        }

                        if (WinAPI.IsIconic(w.Handle))
                        {
                            WinAPI.ShowWindow(w.Handle, WinAPI.SW_RESTORE);
                        }

                        if (WinAPI.SetForegroundWindow(w.Handle))
                        {
                            // Update the selection immediately instead of waiting for the
                            // foreground WinEvent, so a fast second click still minimizes.
                            Form.RememberActiveWindow(w.Handle);
                        }
                    }

                    // Arm a potential window drag. MainForm activates it after the system drag
                    // threshold and draws the semi-transparent icon while dragging.
                    Form.StartWindowDrag(w, VirtualDesktopIndex, e.Location);
                }
            }
        }

        public void OnMouseUp(object sender, MouseEventArgs e)
        {
            // The gesture was a pager drag (Ctrl was held at press time). Even if Ctrl has
            // already been released, this must not select or activate the desktop.
            bool pagerDragGesture = _pagerDragGesture;
            _pagerDragGesture = false;

            if (Form.IsWindowDragActive) return;
            if (!IsInsideBounds(e.X, e.Y)) return;
            if (pagerDragGesture) return;
            if ((Control.ModifierKeys & Keys.Control) != Keys.None) return;

            if ((e.Button & MouseButtons.Left) == MouseButtons.Left)
            {
                if (!IsCurrentActiveDesktop)
                {
                    // Resolve the window under the cursor before switching, the preview data may change afterwards
                    Window windowUnderCursor = Form.Options.DesktopChangeActivation == DesktopChangeActivationMode.WindowUnderCursor
                        ? GetWindowAt(e.Location)
                        : null;

                    // Changing Desktop has to be done when mouse gets up, otherwise we instantly select and focus an
                    // app of the new desktop. Pin first so the pager is visible on the new desktop even when
                    // its taskbar button is hidden.
                    Form.EnsurePinnedToAllDesktops();
                    WindowsVirtualDesktop.GetInstance().FromIndex(VirtualDesktopIndex).MakeVisible();

                    switch (Form.Options.DesktopChangeActivation)
                    {
                        case DesktopChangeActivationMode.WindowUnderCursor:
                            if (windowUnderCursor != null)
                            {
                                Form.ActivateWindowAfterDesktopSwitch(VirtualDesktopIndex, windowUnderCursor.Handle);
                            }

                            break;
                        case DesktopChangeActivationMode.None:
                            break;
                        default:
                            Form.RestoreLastActiveWindow(VirtualDesktopIndex);
                            break;
                    }

                    Form.Invalidate();
                }
            }
        }

        public void OnMouseWheel(object sender, MouseEventArgs e)
        {
            if (!IsInsideBounds(e.X, e.Y)) return;

            // Hacky, so we know the event is faked and came from the context menu
            if ((Control.ModifierKeys & Keys.Shift) != Keys.None || e.Delta == 0)
            {
                // Get current mode for the desktop screen from the RendererViewModes array
                var screenIndex = _screenRenderers.FindIndex(s => e.X >= s.Location.X && e.X <= (s.Location.X + s.Size.Width));
                int mode = (int)Form.Options.RendererViewModes[VirtualDesktopIndex + screenIndex];

                // Change value according wheel direction and trim it to bounds
                mode = e.Delta <= 0 ? mode + 1 : mode - 1;
                int maxValue = Enum.GetValues(typeof(ViewMode)).Length - 1;
                if (mode < 0) mode = maxValue; else if (mode > maxValue) mode = 0;

                //Debug.WriteLine("Changed Mode to: " + mode + " " + (ViewMode)mode);

                // Store the result in the array as well as in the renderer directly
                Form.Options.RendererViewModes[VirtualDesktopIndex + screenIndex] = (ViewMode)mode;
                _screenRenderers[screenIndex].Mode = (ViewMode)mode;

                // Briefly show the new mode above (or below) the affected pager screen
                Form.ShowViewModeHint(_screenRenderers[screenIndex], (ViewMode)mode);

                // Store to registry
                AppSettingsStore.Save(Form.Options);

                // Force a rerender by invalidate the parent MainForm (no need to Reset everything)
                Form.Invalidate();
            }
        }

        public void Render(PaintEventArgs e)
        {
            Graphics g = e.Graphics;

            // N:The Background Color is rendered in the MainForm, so this should be by default darker
            // while the desktop gets another background here
            Color desktopBorderColor = Form.Options.DesktopBorderColor;
            if (IsCurrentActiveDesktop)
            {
                // TODO: Make color and alpha value a settings parameter
                desktopBorderColor = Form.Options.ActiveDesktopBorderColor;

                var background = Color.FromArgb(16, Color.LightBlue);
                using (var selectedDesktopBackgroundColor = new SolidBrush(background))
                {
                    g.FillRectangle(
                        selectedDesktopBackgroundColor,
                        new Rectangle(Location.X, Location.Y, Size.Width, Size.Height));
                }
            }

            // TODO: Make color and maybe alpha value a settings parameter
            var separaterColor = Color.FromArgb(64, Color.LightBlue);

            using (var separatorBrush = new SolidBrush(separaterColor))
            {
                int xoffset = 0;
                _screenRenderers.ForEach(renderer =>
                {
                    // Trigger repaint of each renderer
                    renderer.Render(e);

                    // Draw separators when more than one screens are present
                    if (_numScreens > 1)
                    {
                        xoffset += renderer.Size.Width;
                        g.FillRectangle(
                            separatorBrush,
                            new Rectangle(Location.X + xoffset, Location.Y, Form.BorderSize, Size.Height - Form.BorderSize));
                    }
                });
            }

            using (var desktopBorderBrush = new SolidBrush(desktopBorderColor))
            {
                if (Form.Options.DesktopBorderStyle == DesktopBorderStyle.Box)
                {
                    // GDI+ centers the pen stroke on the rectangle path. Inset the path by half the
                    // border width, otherwise the outer half falls outside the desktop bounds and the
                    // border looks incomplete (one pixel missing at the right and bottom edge) at
                    // DPI scale factors where BorderSize is greater than 1.
                    int borderInset = Form.BorderSize / 2;

                    // Draw a border around the active desktop
                    using (var desktopBorderPen = new Pen(desktopBorderBrush, Form.BorderSize))
                    {
                        g.DrawRectangle(
                            desktopBorderPen,
                            new Rectangle(
                                Location.X + borderInset,
                                Location.Y + borderInset,
                                Size.Width - Form.BorderSize,
                                Size.Height - Form.BorderSize));
                    }
                }
                else
                {
                    // Just underline the active desktop
                    g.FillRectangle(
                        desktopBorderBrush,
                        new Rectangle(Location.X, Size.Height - Form.BorderSize, Size.Width, Size.Height));
                }
            }
        }

        public Window GetWindowAt(Point clientPoint)
        {
            // Each screen renderer can show a different view mode. The hit test must use the
            // renderer's actual mode: Preview stacks windows, so only there the z-order decides
            // which overlapping window was clicked. Using the configured default mode instead
            // picked the wrong window whenever the mode had been toggled at runtime.
            foreach (var renderer in _screenRenderers)
            {
                var windows = Form.Windows
                    .Where(x => renderer.WindowAreas.ContainsKey(x.Handle));

                var orderedWindows = renderer.Mode == ViewMode.Preview
                    ? windows.OrderByDescending(x => x.ZOrder)
                    : windows.OrderBy(x => x.ProcessID).ThenBy(x => x.Handle.ToInt64());

                foreach (var w in orderedWindows)
                {
                    if (renderer.WindowAreas[w.Handle].Contains(clientPoint))
                    {
                        return w;
                    }
                }
            }

            return null;
        }

        private bool IsInsideBounds(Point p) => IsInsideBounds(p.X, p.Y);

        private bool IsInsideBounds(int x, int y)
            => x >= Location.X && x < (Location.X + Size.Width) && y >= Location.Y && y < (Location.Y + Size.Height);

        // Resolves which view mode the screen at the given x coordinate currently shows
        private bool IsMicroTaskbarAt(int x)
        {
            int screenIndex = _screenRenderers.FindIndex(
                s => x >= s.Location.X && x < (s.Location.X + s.Size.Width));
            return screenIndex >= 0 && _screenRenderers[screenIndex].Mode == ViewMode.MicroTaskbar;
        }
    }
}
