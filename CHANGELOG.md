# Changelog

## v1.5.0 (not released yet)

- project decoupled from the long abandoned Switchie project, rebranded to *Micro Task Pager* with a different develeping focus
- Desktops: complete rewrite of the MicroTaskbar renderer
- App: added prevention against multiple app instances
- App: removed paddingSize parameter
- Settings Dialog: Live preview now also works for pagerHeight parameter
- Settings Dialog: Added numeric input control for easy fine granular changes
- Settings Dialog: Better organization of all parameters
- Settings Dialog: Reset to defaults now works for each tab individually
- Settings Dialog: Allow revert single parameters to previous values instead of defaults
- Internal: Added automatic build versions
- Internal: Added StyleCop.Analyzer width customized settings
- Internal: Multiple code refactorings


## v1.4.4

Bug fixes:
- Get it run on more recent Windows 11 versions 24H2, 25H2 


## v1.4.3

Changes:
- Changed version management to have the application manifest as source so also the executable is correctly versioned


## v1.4.2

New features:
- Setting to hide application window from Taskbar
- Setting to change opacity of the application

Bug fixes
- Filter minimized windows from list, especially important for the icons render mode
- Clicking on windows in Icons render mode now works reliably when an application has more than one window


## v1.4.0

Bug fixes:
- Get it run on recent Windows 11 version again

Changes:
- Instance creation has now several fallbacks so it should work on several Win 10 and Win 11 versions 


## v1.3.3

Changes:
- no longer allow mouse clicks to bring windows from another desktop to foreground
- Added tooltips / infos in the settings dialog for some of the settings which need an explanation


## v1.3.2

New features:
- Icons render mode supports a second row when there isn't enough space in one row
- Add parameters for desktop padding and icon padding in settings dialog

Changes:
- fixing performance properties
- layout improvements in settings dialog
- app versioning increase now less aggressive


## v1.3.1

Changes:
- Icons render mode now also supports selection as well as drag & drop to other desktops
- the context menu now also has some nice icons based on glyhps


## v1.3.0

New features:
- new settings dialog

Changes:
- nicer about dialog

Others:
- removed not used properties and functions


## v1.2.0

Bug fixes:
- Update application size when number of desktops are changed

New features:
- New alternative render mode: Show a list of application icons instead windows
- Save render mode current window position (on request) to registry and restore them from there on startup
- Windows can be brought to front when clicking on them

Others:
- Some refactoring and cleanup
