using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Linq;
using System.Threading.Tasks;
using System.Windows.Forms;
using MicroTaskPager.Config;
using MicroTaskPager.Core;
using MicroTaskPager.Core.API;
using BorderStyle = MicroTaskPager.Config.BorderStyle;

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
        private DragDropData _dragDropData;
        private Rectangle dragBoxFromMouseDown;

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
            if (!IsInsideBounds(e.X, e.Y)) return;
            if ((Control.ModifierKeys & Keys.Shift) != Keys.None) return;
            if ((e.Button & MouseButtons.Left) == MouseButtons.Left)
            {
                var w = GetWindowUnderCursor(e.Location);
                if (w != null)
                {
                    // Prepare drag & drop for the selected window (even if it's not in the users mind)
                    _dragDropData = new DragDropData()
                    {
                        OriginDesktopIndex = VirtualDesktopIndex,
                        DraggedWindow = w,
                    };
                    Size dragSize = SystemInformation.DragSize;
                    dragBoxFromMouseDown = new Rectangle(
                        new Point(e.X - (dragSize.Width / 2), e.Y - (dragSize.Height / 2)), dragSize);
                }
                else
                {
                    dragBoxFromMouseDown = Rectangle.Empty;
                }
            }
        }

        public void OnMouseUp(object sender, MouseEventArgs e)
        {
            if (!IsInsideBounds(e.X, e.Y)) return;
            if ((Control.ModifierKeys & Keys.Shift) != Keys.None) return;
            if ((e.Button & MouseButtons.Left) == MouseButtons.Left)
            {
                // Bring a window into front by clicking on its miniature / icon
                // -> only when it's already the active desktop, which makes desktop switches smoother
                //    without touching the last window z-order
                if (IsCurrentActiveDesktop)
                {
                    var w = GetWindowUnderCursor(e.Location);
                    if (w != null)
                    {
                        WinAPI.SetForegroundWindow(w.Handle);
                    }
                }
                else
                {
                    // Changing Desktop has to be done when mouse gets up, otherwise we instantly select and focus an
                    // app of the new desktop
                    WindowsVirtualDesktop.GetInstance().FromIndex(VirtualDesktopIndex).MakeVisible();
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
                mode = e.Delta > 0 ? mode + 1 : mode - 1;
                int maxValue = Enum.GetValues(typeof(ViewMode)).Length - 1;
                if (mode < 0) mode = maxValue; else if (mode > maxValue) mode = 0;

                // Store the result in the array as well as in the renderer directly
                Form.Options.RendererViewModes[VirtualDesktopIndex + screenIndex] = (ViewMode)mode;
                _screenRenderers[screenIndex].Mode = (ViewMode)mode;

                // Store to registry
                AppSettingsStore.Save(Form.Options);

                // Force a rerender by invalidate the parent MainForm (no need to Reset everything)
                Form.Invalidate();
            }
        }

        public void OnMouseMove(object sender, MouseEventArgs e)
        {
            if (!IsInsideBounds(e.X, e.Y)) return;
            if ((Control.ModifierKeys & Keys.Shift) != Keys.None) return;

            if ((e.Button & MouseButtons.Left) == MouseButtons.Left)
            {
                // The previously prepared drag & drop gets activated
                if (dragBoxFromMouseDown != Rectangle.Empty && !dragBoxFromMouseDown.Contains(e.X, e.Y))
                    Form.DoDragDrop(_dragDropData, DragDropEffects.Move);
            }
        }

        // While Dragging...
        public void OnDragOver(object sender, DragEventArgs e)
        {
            if (!e.Data.GetDataPresent(typeof(DragDropData)))
            {
                e.Effect = DragDropEffects.None;
                return;
            }

            if ((e.AllowedEffect & DragDropEffects.Move) == DragDropEffects.Move)
                e.Effect = DragDropEffects.Move;
        }

        // Move window to another desktop when dropped
        public void OnDragDrop(object sender, DragEventArgs e)
        {
            if (!IsInsideBounds(Form.PointToClient(new Point(e.X, e.Y)))) return;
            if (e.Data.GetDataPresent(typeof(DragDropData)))
            {
                var ddd = (DragDropData)e.Data.GetData(typeof(DragDropData));
                if (e.Effect == DragDropEffects.Move)
                {
                    WindowsVirtualDesktop.GetInstance().FromIndex(VirtualDesktopIndex).MoveWindow(ddd.DraggedWindow.Handle);
                    Form.Invalidate();
                }
            }
        }

        public void Render(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            var separaterColor = Color.FromArgb(64, Color.LightBlue); // TODO: Make color and maybe alpha value a settings parameter

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

            Color desktopBorderColor = IsCurrentActiveDesktop ? Form.Options.ActiveDesktopBorderColor : Form.Options.DesktopBorderColor;

            using (var desktopBorderBrush = new SolidBrush(desktopBorderColor))
            {
                if (Form.Options.DesktopBorderStyle == BorderStyle.Box)
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

        private bool IsInsideBounds(Point p) => IsInsideBounds(p.X, p.Y);

        private bool IsInsideBounds(int x, int y)
            => x >= Location.X && x < (Location.X + Size.Width) && y >= Location.Y && y < (Location.Y + Size.Height);

        private Window GetWindowUnderCursor(Point mousePosition)
        {
            var coord = Form.PointToClient(mousePosition);
            var visibleWindows = Form.Windows
                .Where(x => x.VirtualDesktopIndex == VirtualDesktopIndex && !WinAPI.IsIconic(x.Handle));

            // For desktop preview mode, we search across all screens for the clicked window in z-order from front to back,
            // otherwise order should stay the same (using process id & handle)
            var windows = Form.Options.ViewModeDefault == ViewMode.Preview
                ? visibleWindows.OrderByDescending(x => x.ZOrder)
                : visibleWindows
                    .OrderBy(x => x.ProcessID)
                    .ThenBy(x => x.Handle.ToInt64());

            foreach (var w in windows)
            {
                if (_screenRenderers.Any(x => x.WindowAreas.ContainsKey(w.Handle) && x.WindowAreas[w.Handle].Contains(mousePosition)))
                {
                    if (_screenRenderers.Select(x => x.AttachedScreen.DeviceName).Contains(Screen.FromHandle(w.Handle).DeviceName))
                    {
                        return w;
                    }
                }
            }

            return null;
        }
    }
}
