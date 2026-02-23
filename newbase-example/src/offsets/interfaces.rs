// Generated using https://github.com/a2x/cs2-dumper
// 2026-02-21 03:35:19.903985300 UTC

#![allow(non_upper_case_globals, unused)]

pub mod cs2_dumper {
    pub mod interfaces {
        // Module: animationsystem.dll
        pub mod animationsystem_dll {
            pub const AnimationSystemUtils_001: usize = 0x7F2470;
            pub const AnimationSystem_001: usize = 0x7EA390;
        }
        // Module: client.dll
        pub mod client_dll {
            pub const ClientToolsInfo_001: usize = 0x2E6F4C0;
            pub const GameClientExports001: usize = 0x2E6C1A0;
            pub const Source2Client002: usize = 0x373DE80;
            pub const Source2ClientConfig001: usize = 0x323BE30;
            pub const Source2ClientPrediction001: usize = 0x2E76F10;
            pub const Source2ClientUI001: usize = 0x2E86300;
        }
        // Module: engine2.dll
        pub mod engine2_dll {
            pub const BenchmarkService001: usize = 0x6238F0;
            pub const BugService001: usize = 0x8DA170;
            pub const ClientServerEngineLoopService_001: usize = 0x91B810;
            pub const EngineGameUI001: usize = 0x6212F0;
            pub const EngineServiceMgr001: usize = 0x91B0D0;
            pub const GameEventSystemClientV001: usize = 0x91B3B0;
            pub const GameEventSystemServerV001: usize = 0x91B4E0;
            pub const GameResourceServiceClientV001: usize = 0x6239F0;
            pub const GameResourceServiceServerV001: usize = 0x623A50;
            pub const GameUIService_001: usize = 0x8DA5A0;
            pub const HostStateMgr001: usize = 0x624270;
            pub const INETSUPPORT_001: usize = 0x61C920;
            pub const InputService_001: usize = 0x8DA890;
            pub const KeyValueCache001: usize = 0x624320;
            pub const MapListService_001: usize = 0x919700;
            pub const NetworkClientService_001: usize = 0x919890;
            pub const NetworkP2PService_001: usize = 0x919BD0;
            pub const NetworkServerService_001: usize = 0x919D80;
            pub const NetworkService_001: usize = 0x623BC0;
            pub const RenderService_001: usize = 0x919FF0;
            pub const ScreenshotService001: usize = 0x91A2B0;
            pub const SimpleEngineLoopService_001: usize = 0x624380;
            pub const SoundService_001: usize = 0x623C00;
            pub const Source2EngineToClient001: usize = 0x620C10;
            pub const Source2EngineToClientStringTable001: usize = 0x620C70;
            pub const Source2EngineToServer001: usize = 0x620CE8;
            pub const Source2EngineToServerStringTable001: usize = 0x620D10;
            pub const SplitScreenService_001: usize = 0x623EE0;
            pub const StatsService_001: usize = 0x91A670;
            pub const ToolService_001: usize = 0x6240A0;
            pub const VENGINE_GAMEUIFUNCS_VERSION005: usize = 0x621380;
            pub const VProfService_001: usize = 0x6240E0;
        }
        // Module: filesystem_stdio.dll
        pub mod filesystem_stdio_dll {
            pub const VAsyncFileSystem2_001: usize = 0x215970;
            pub const VFileSystem017: usize = 0x215730;
        }
        // Module: host.dll
        pub mod host_dll {
            pub const DebugDrawQueueManager001: usize = 0x138F80;
            pub const GameModelInfo001: usize = 0x138FC0;
            pub const GameSystem2HostHook: usize = 0x139000;
            pub const HostUtils001: usize = 0x146640;
            pub const PredictionDiffManager001: usize = 0x139110;
            pub const SaveRestoreDataVersion001: usize = 0x139240;
            pub const SinglePlayerSharedMemory001: usize = 0x139270;
            pub const Source2Host001: usize = 0x1392E0;
        }
        // Module: imemanager.dll
        pub mod imemanager_dll {
            pub const IMEManager001: usize = 0x36AA0;
        }
        // Module: inputsystem.dll
        pub mod inputsystem_dll {
            pub const InputStackSystemVersion001: usize = 0x40DD0;
            pub const InputSystemVersion001: usize = 0x42AD0;
        }
        // Module: localize.dll
        pub mod localize_dll {
            pub const Localize_001: usize = 0x56E20;
        }
        // Module: materialsystem2.dll
        pub mod materialsystem2_dll {
            pub const FontManager_001: usize = 0x165620;
            pub const MaterialUtils_001: usize = 0x14D500;
            pub const PostProcessingSystem_001: usize = 0x14D410;
            pub const TextLayout_001: usize = 0x14D490;
            pub const VMaterialSystem2_001: usize = 0x164F10;
        }
        // Module: meshsystem.dll
        pub mod meshsystem_dll {
            pub const MeshSystem001: usize = 0x14F6A0;
        }
        // Module: navsystem.dll
        pub mod navsystem_dll {
            pub const NavSystem001: usize = 0x1219E0;
        }
        // Module: networksystem.dll
        pub mod networksystem_dll {
            pub const FlattenedSerializersVersion001: usize = 0x2746F0;
            pub const NetworkMessagesVersion001: usize = 0x29C760;
            pub const NetworkSystemVersion001: usize = 0x28DEA0;
            pub const SerializedEntitiesVersion001: usize = 0x28DF90;
        }
        // Module: panorama.dll
        pub mod panorama_dll {
            pub const PanoramaUIEngine001: usize = 0x508CB0;
        }
        // Module: panorama_text_pango.dll
        pub mod panorama_text_pango_dll {
            pub const PanoramaTextServices001: usize = 0x2B89C0;
        }
        // Module: panoramauiclient.dll
        pub mod panoramauiclient_dll {
            pub const PanoramaUIClient001: usize = 0x293380;
        }
        // Module: particles.dll
        pub mod particles_dll {
            pub const ParticleSystemMgr003: usize = 0x52B890;
        }
        // Module: pulse_system.dll
        pub mod pulse_system_dll {
            pub const IPulseSystem_001: usize = 0x1F2750;
        }
        // Module: rendersystemdx11.dll
        pub mod rendersystemdx11_dll {
            pub const RenderDeviceMgr001: usize = 0x431D30;
            pub const RenderUtils_001: usize = 0x432628;
            pub const VRenderDeviceMgrBackdoor001: usize = 0x431DD0;
        }
        // Module: resourcesystem.dll
        pub mod resourcesystem_dll {
            pub const ResourceSystem013: usize = 0x82F60;
        }
        // Module: scenefilecache.dll
        pub mod scenefilecache_dll {
            pub const ResponseRulesCache001: usize = 0xE1750;
            pub const SceneFileCache002: usize = 0xE1878;
        }
        // Module: scenesystem.dll
        pub mod scenesystem_dll {
            pub const RenderingPipelines_001: usize = 0x65BAC0;
            pub const SceneSystem_002: usize = 0x8D0260;
            pub const SceneUtils_001: usize = 0x65C9D0;
        }
        // Module: schemasystem.dll
        pub mod schemasystem_dll {
            pub const SchemaSystem_001: usize = 0x76780;
        }
        // Module: server.dll
        pub mod server_dll {
            pub const EntitySubclassUtilsV001: usize = 0x2FE4110;
            pub const NavGameTest001: usize = 0x3133F38;
            pub const ServerToolsInfo_001: usize = 0x30E1BE8;
            pub const Source2GameClients001: usize = 0x30DD230;
            pub const Source2GameDirector001: usize = 0x34B7A50;
            pub const Source2GameEntities001: usize = 0x30E12E0;
            pub const Source2Server001: usize = 0x30E1150;
            pub const Source2ServerConfig001: usize = 0x34FE0E8;
        }
        // Module: soundsystem.dll
        pub mod soundsystem_dll {
            pub const SoundOpSystem001: usize = 0x50BCA0;
            pub const SoundOpSystemEdit001: usize = 0x50BB60;
            pub const SoundSystem001: usize = 0x50B650;
            pub const VMixEditTool001: usize = 0x594868F;
        }
        // Module: steamaudio.dll
        pub mod steamaudio_dll {
            pub const SteamAudio001: usize = 0x25D580;
        }
        // Module: steamclient64.dll
        pub mod steamclient64_dll {
            pub const IVALIDATE001: usize = 0x166F0A8;
            pub const SteamClient006: usize = 0x166C5B0;
            pub const SteamClient007: usize = 0x166C5B8;
            pub const SteamClient008: usize = 0x166C5C0;
            pub const SteamClient009: usize = 0x166C5C8;
            pub const SteamClient010: usize = 0x166C5D0;
            pub const SteamClient011: usize = 0x166C5D8;
            pub const SteamClient012: usize = 0x166C5E0;
            pub const SteamClient013: usize = 0x166C5E8;
            pub const SteamClient014: usize = 0x166C5F0;
            pub const SteamClient015: usize = 0x166C5F8;
            pub const SteamClient016: usize = 0x166C600;
            pub const SteamClient017: usize = 0x166C608;
            pub const SteamClient018: usize = 0x166C610;
            pub const SteamClient019: usize = 0x166C618;
            pub const SteamClient020: usize = 0x166C620;
            pub const SteamClient021: usize = 0x166C628;
            pub const SteamClient022: usize = 0x166C630;
            pub const SteamClient023: usize = 0x166C638;
            pub const p2pvoice002: usize = 0x14E4E6F;
            pub const p2pvoicesingleton002: usize = 0x16480F0;
        }
        // Module: tier0.dll
        pub mod tier0_dll {
            pub const TestScriptMgr001: usize = 0x39A670;
            pub const VEngineCvar007: usize = 0x3A5470;
            pub const VProcessUtils002: usize = 0x39A610;
            pub const VStringTokenSystem001: usize = 0x3CC160;
        }
        // Module: v8system.dll
        pub mod v8system_dll {
            pub const Source2V8System001: usize = 0x316B0;
        }
        // Module: vphysics2.dll
        pub mod vphysics2_dll {
            pub const VPhysics2_Handle_Interface_001: usize = 0x4054D0;
            pub const VPhysics2_Interface_001: usize = 0x405510;
        }
        // Module: vscript.dll
        pub mod vscript_dll {
            pub const VScriptManager010: usize = 0x13B390;
        }
        // Module: vstdlib_s64.dll
        pub mod vstdlib_s64_dll {
            pub const IVALIDATE001: usize = 0x6E990;
            pub const VEngineCvar002: usize = 0x6D070;
        }
        // Module: worldrenderer.dll
        pub mod worldrenderer_dll {
            pub const WorldRendererMgr001: usize = 0x223F90;
        }
    }
}
