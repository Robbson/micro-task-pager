using System;
using System.Drawing;
using System.IO;
using System.Reflection;
using System.Windows.Forms;

namespace MicroTaskPager.Core
{
    public class Helpers
    {
        public static Bitmap CreateGlyphBitmap(string glyph, int size = 16)
        {
            var bmp = new Bitmap(size, size);
            using (var g = Graphics.FromImage(bmp))
            {
                g.Clear(Color.Transparent);
                g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAliasGridFit;

                // Try common Windows fonts with broad unicode support.
                using (var font = new Font("Segoe UI Symbol", size - 2, FontStyle.Regular, GraphicsUnit.Pixel))
                using (var sf = new StringFormat
                {
                    Alignment = StringAlignment.Center,
                    LineAlignment = StringAlignment.Center,
                    FormatFlags = StringFormatFlags.NoWrap,
                })
                {
                    g.DrawString(glyph, font, Brushes.Black, new RectangleF(0, 0, size, size), sf);
                }
            }

            return bmp;
        }

        public static byte[] GetResourceFromAssembly(Type type, string name)
        {
            MemoryStream ms = new MemoryStream();
            Assembly.GetAssembly(type).GetManifestResourceStream(name).CopyTo(ms);
            return ms.ToArray();
        }

        public static void AddMenuItem(Form main, ContextMenuStrip menu, ToolStripMenuItem m, Action onClick = null)
        {
            m.Click += (s, e) => onClick?.Invoke();
            menu.Items.Add(m);
        }

        public static Size AspectRatioResize(Size sz, int finalWidth, int finalHeight)
        {
            int iWidth;
            int iHeight;
            if ((finalHeight == 0) && (finalWidth != 0))
            {
                iWidth = finalWidth;
                iHeight = sz.Height * iWidth / sz.Width;
            }
            else if ((finalHeight != 0) && (finalWidth == 0))
            {
                iHeight = finalHeight;
                iWidth = sz.Width * iHeight / sz.Height;
            }
            else
            {
                iWidth = finalWidth;
                iHeight = finalHeight;
            }

            return new Size(iWidth, iHeight);
        }
    }
}
