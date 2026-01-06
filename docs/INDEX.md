# Documentation Index

Welcome to the Rustcraft documentation! This index will help you find the information you need.

## 📖 Getting Started

If you're new to the project:

1. **Start here**: [README.md](../README.md) - Project overview, installation, and basic controls
2. **Then read**: [DEVELOPER_GUIDE.md](../DEVELOPER_GUIDE.md) - Setup, workflow, and common tasks
3. **Understand the structure**: [ARCHITECTURE.md](../ARCHITECTURE.md) - System design and organization

## 📚 Documentation Structure

```
rustcraft/
├── README.md                    # Project overview and getting started
├── ARCHITECTURE.md              # System architecture and design
├── DEVELOPER_GUIDE.md           # Developer onboarding and workflows
└── docs/
    ├── BIG_SPACE_CHUNK_SYSTEM_PLAN.md  # Big Space chunk system migration
    ├── FRUSTUM_CULLING_PLAN.md  # View frustum culling optimization
    ├── LOD_SYSTEM_PLAN.md       # Level of Detail rendering system
    └── modules/                 # Detailed module documentation
        ├── WORLD_SYSTEM.md      # World generation and management
        ├── NETWORK_SYSTEM.md    # Multiplayer networking
        ├── RENDERING_SYSTEM.md  # Graphics and UI
        └── PLAYER_ENTITY_SYSTEMS.md  # Players, mobs, and items
```

## 🎯 Find What You Need

### I want to...

**Understand the codebase**
- Read [ARCHITECTURE.md](../ARCHITECTURE.md) for the big picture
- Review module docs for specific systems

**Set up my development environment**
- Follow [DEVELOPER_GUIDE.md](../DEVELOPER_GUIDE.md) → Getting Started

**Add a new feature**
- [DEVELOPER_GUIDE.md](../DEVELOPER_GUIDE.md) → Common Development Tasks
- Relevant module documentation for the system you're modifying

**Fix a bug**
- [DEVELOPER_GUIDE.md](../DEVELOPER_GUIDE.md) → Testing and Debugging
- [DEVELOPER_GUIDE.md](../DEVELOPER_GUIDE.md) → Troubleshooting

**Understand world generation**
- [WORLD_SYSTEM.md](./modules/WORLD_SYSTEM.md)

**Optimize chunk rendering**
- [FRUSTUM_CULLING_PLAN.md](./FRUSTUM_CULLING_PLAN.md) - View culling
- [LOD_SYSTEM_PLAN.md](./LOD_SYSTEM_PLAN.md) - Level of Detail
- [BIG_SPACE_CHUNK_SYSTEM_PLAN.md](./BIG_SPACE_CHUNK_SYSTEM_PLAN.md) - Floating origin precision system

**Work with networking**
- [NETWORK_SYSTEM.md](./modules/NETWORK_SYSTEM.md)

**Modify rendering or UI**
- [RENDERING_SYSTEM.md](./modules/RENDERING_SYSTEM.md)

**Change player mechanics or AI**
- [PLAYER_ENTITY_SYSTEMS.md](./modules/PLAYER_ENTITY_SYSTEMS.md)

## 📊 Documentation Statistics

- **Total documentation**: ~4,500 lines
- **Core documents**: 3 files
- **Module guides**: 4 files
- **Coverage**: All major systems documented

## 🔍 Quick Reference

### Key Concepts

- **ECS (Entity Component System)**: Bevy's architecture pattern
- **Client-Server**: Authoritative server design
- **Chunks**: 16x16x256 world sections
- **Greedy Meshing**: Polygon optimization for voxels
- **bevy_renet**: Networking library

### Important Paths

```
client/src/     # Client-side code (rendering, UI, input)
server/src/     # Server-side code (world, logic, AI)
shared/src/     # Shared code (messages, data structures)
data/           # Game assets (textures, etc.)
```

### Key Files

```
client/src/main.rs              # Client entry point
server/src/main.rs              # Server entry point
shared/src/lib.rs               # Shared library root
shared/src/world/blocks.rs      # Block definitions
shared/src/messages/mod.rs      # Network messages
client/src/world/rendering/meshing.rs  # Mesh generation
server/src/world/generation.rs  # World generation
```

## 🔗 External Resources

### Bevy Engine
- [Official Bevy Book](https://bevyengine.org/learn/book/introduction/)
- [Bevy API Docs](https://docs.rs/bevy/)
- [Bevy Cheat Book](https://bevy-cheatbook.github.io/)

### Rust Language
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)

### Voxel Game Development
- [Greedy Meshing Article](https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/)
- [Voxel Engine Basics](https://www.youtube.com/watch?v=Ab8TOSFfNp4)

## 🤝 Contributing

Before contributing:

1. Read [DEVELOPER_GUIDE.md](../DEVELOPER_GUIDE.md) → Contributing Guidelines
2. Check the [Contributing section in README.md](../README.md#contributing)
3. Follow the code style and commit message conventions

## 📝 Documentation Maintenance

### Updating Documentation

When you make code changes:
- Update relevant documentation if behavior changes
- Add examples for new features
- Keep code snippets in sync with actual code

### Documentation Style

- Use clear, concise language
- Include code examples
- Add diagrams where helpful
- Link to related sections
- Keep formatting consistent

## 🆘 Getting Help

If you can't find what you need:
1. Check the [Troubleshooting sections](../DEVELOPER_GUIDE.md#troubleshooting) in the guides
2. Search the codebase for examples
3. Open a GitHub Discussion
4. Ask in the community Discord (if available)

---

**Last Updated**: 2025-12-22

**Documentation Version**: 1.0

**Codebase Version**: Compatible with main branch
