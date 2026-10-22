using System;
using System.Drawing;
using System.IO;
using System.Windows.Forms;
using MicroTaskPager.Core;

namespace MicroTaskPager.GUI
{
    public class AboutDialog : Form
    {
        private readonly string _version =
            typeof(AboutDialog).Assembly.GetName().Version?.ToString(3)
            ?? Application.ProductVersion;

        public AboutDialog()
        {
            SuspendLayout();

            Text = "About Micro Task Pager";
            FormBorderStyle = FormBorderStyle.FixedDialog;
            StartPosition = FormStartPosition.CenterParent;
            MaximizeBox = false;
            MinimizeBox = false;
            ShowInTaskbar = false;
            ClientSize = new Size(420, 340);
            BackColor = Color.White;

            var iconBox = new PictureBox
            {
                Size = new Size(96, 96),
                SizeMode = PictureBoxSizeMode.Zoom,
                Location = new Point((ClientSize.Width - 96) / 2, 20),
            };

            try
            {
                using (var iconStream = new MemoryStream(
                    Helpers.GetResourceFromAssembly(typeof(Program), "MicroTaskPager.Resources.icon.png")))
                using (var pngImage = Image.FromStream(iconStream))
                {
                    iconBox.Image = new Bitmap(pngImage);
                }
            }
            catch
            {
                iconBox.Image = Icon?.ToBitmap();
            }

            var appNameLabel = new Label
            {
                AutoSize = false,
                Width = ClientSize.Width,
                Height = 32,
                Location = new Point(0, 126),
                TextAlign = ContentAlignment.MiddleCenter,
                Font = new Font("Segoe UI", 16, FontStyle.Bold),
                Text = "Micro Task Pager",
            };

            var versionLabel = new Label
            {
                AutoSize = false,
                Width = ClientSize.Width,
                Height = 24,
                Location = new Point(0, 160),
                TextAlign = ContentAlignment.MiddleCenter,
                Font = new Font("Segoe UI", 10, FontStyle.Regular),
                ForeColor = Color.DimGray,
                Text = $"Version {_version}",
            };

            var authorLabel = new Label
            {
                AutoSize = false,
                Width = ClientSize.Width - 40,
                Height = 24,
                Location = new Point(20, 202),
                TextAlign = ContentAlignment.MiddleLeft,
                Font = new Font("Segoe UI", 9, FontStyle.Bold),
                Text = "Author: Robbson (Robin Siegemund) 2026",
            };

            var contributorsLabel = new Label
            {
                AutoSize = false,
                Width = ClientSize.Width - 40,
                Height = 70,
                Location = new Point(20, 226),
                TextAlign = ContentAlignment.TopLeft,
                Font = new Font("Segoe UI", 9, FontStyle.Regular),
                Text = "Acknowledgments:\n" +
                " • DARKGuy (original project)\n" +
                " • MScholtes (Virtual Desktop)",
            };

            var closeButton = new Button
            {
                Text = "Close",
                Width = 100,
                Height = 30,
                Location = new Point((ClientSize.Width - 100) / 2, 298),
                DialogResult = DialogResult.OK,
            };

            AcceptButton = closeButton;
            CancelButton = closeButton;

            Controls.Add(iconBox);
            Controls.Add(appNameLabel);
            Controls.Add(versionLabel);
            Controls.Add(authorLabel);
            Controls.Add(contributorsLabel);
            Controls.Add(closeButton);

            ResumeLayout(false);
        }
    }
}
