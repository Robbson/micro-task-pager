# Changelog

## v1.5.0

### General
- project decoupled from the long abandoned Switchie project, rebranded to *Micro Task Pager* with a different developing focus
- all new icon and logo, updated about dialog 
- added file logging for easier support in case of errors
- multiple starts of the app (with the same user) are now prevented
- fixed app shutdown crash
- improved application exit

### Pager
- support of high-DPI screens
- added individual view modes for each desktop's screen
- added pager movement with shift + left mouse button as an alternative to mouse middle-button dragging
- each desktop now remembers it's last active window and reactivates it when the users comes back to it
- complete rewrite of the MicroTaskbar renderer for a cleaner look on opened windows at any pager size
- screen (dis-)connections to the PC while the app is running are now recognized, no restart required
- shows a separator line in case of multiple screens
- show pager dragging cursor already on click
- fixed missing selection for windows of the same process
- fixed icons centering routines and pageHeight rendering preview
- fixed memory leaks regarding GDI objects and fonts in the main form
- fixed annoying Window activation before drag & drop operations
- removed no longer needed paddingSize parameter
- optimized redraws

### Settings Dialog
- better organization of all changeable parameters
- updated settings dialog positioning next to the pager instead of over it
- allow revert single parameters to previous values instead of defaults
- reset to defaults now works for each tab individually
- live preview now also works for pagerHeight parameter
- added numeric input control for easy fine granular changes
- updated numeric controls with min/max limits
- restrict pagerHeight minimum height including padding
- fixed out of bounds exception when opening settings window

### Internal
- added automatic build versions
- added StyleCop.Analyzer width customized settings
- improved exception handling;  
- updated prod build for zip archive generation 
- tons of code refactorings


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
