# Rust game manager

This repository is responsible for the setup of the various games.

## TODO

- [ ] Take all the assets from other repositories and store them in a folder
Mainly to strip assets from godot projects.
What I mean by assets are these files:
Images
- .png
- .xcf
- .jpg

3D models
- .blend
- .blend1
- .glb

Sounds
- .ogg

Fonts
- .ttf

- [ ] It must preserve subfolder hierarchy
E.g:
Project at root has files like this
- assets
    - images
        - .png
        - .jpg
    - 3d_models
        - .blend
        - .blend1
        - .glb
    - sounds
        - .ogg

