
To debug messages use this command in a console:
winsocat STDIO NPIPE:DirMon

To install winsocat: winget install winsocat

```
# This is an example configuration file
#  copy and modify it as needed

pipe_name = "DirMon"
logfile = ".\\dirmon.log"

["C:\\Users\\manzogi9\\Sviluppo\\dirmon"]
Create = "Action"

["C:\\Users\\manzogi9\\Sviluppo\\dirmon\\Cargo.toml"]
Create = "Action"
```

