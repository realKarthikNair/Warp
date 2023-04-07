# Copy appdata to gresource dir
appdata_gresource = custom_target(
    'metainfo.xml',
    output: 'metainfo.xml',
    depends: appdata_file,
    build_by_default: true,
    command: [
        'cp',
        appdata_file.full_path(),
        meson.project_source_root() / 'data' / 'resources' / 'metainfo.xml'
    ]
)