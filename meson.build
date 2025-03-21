project(
  run_command(
    find_program(meson.global_source_root() / 'build-aux/meson-cargo-manifest.py'),
    'package',
    'name',
    check: true,
  ).stdout(),
  'rust',
  version: run_command(
    find_program(meson.global_source_root() / 'build-aux/meson-cargo-manifest.py'),
    'package',
    'version',
    check: true,
  ).stdout(),
  meson_version: '>= 1.1.0',
  license: run_command(
    find_program(meson.global_source_root() / 'build-aux/meson-cargo-manifest.py'),
    'package',
    'license',
    check: true,
  ).stdout(),
)

homepage = run_command(
  find_program(meson.global_source_root() / 'build-aux/meson-cargo-manifest.py'),
  'package',
  'homepage',
  check: true,
).stdout()
issuetracker = run_command(
  find_program(meson.global_source_root() / 'build-aux/meson-cargo-manifest.py'),
  'package',
  'metadata',
  'urls',
  'issuetracker',
  check: true,
).stdout()
contact = run_command(
  find_program(meson.global_source_root() / 'build-aux/meson-cargo-manifest.py'),
  'package',
  'metadata',
  'urls',
  'contact',
  check: true,
).stdout()
repository = run_command(
  find_program(meson.global_source_root() / 'build-aux/meson-cargo-manifest.py'),
  'package',
  'repository',
  check: true,
).stdout()

i18n = import('i18n')
gnome = import('gnome')

base_id = 'app.drey.Warp'

dependency('glib-2.0', version: '>= 2.80.0')
dependency('gio-2.0', version: '>= 2.80.0')
dependency('gtk4', version: '>= 4.18.0')
dependency('libadwaita-1', version: '>= 1.7')

# QR Code scanning

linux = get_option('target') == '' or get_option('target').contains('linux')
qr_code_scanning_feat = get_option('qr-code-scanning').disable_if(not linux)

qr_code_deps = [
  dependency('gstreamer-1.0', version: '>= 1.20', required: qr_code_scanning_feat),
  dependency('gstreamer-base-1.0', version: '>= 1.20', required: qr_code_scanning_feat),
  dependency(
    'gstreamer-plugins-base-1.0',
    version: '>= 1.20',
    required: qr_code_scanning_feat,
  ),
  dependency(
    'gstreamer-plugins-bad-1.0',
    version: '>= 1.20',
    required: qr_code_scanning_feat,
  ),
]

qr_code_scanning = qr_code_scanning_feat.allowed()
foreach dep : qr_code_deps
  qr_code_scanning = qr_code_scanning and dep.found()
endforeach

glib_compile_schemas = find_program('glib-compile-schemas', required: true)
desktop_file_validate = find_program('desktop-file-validate', required: false)
appstream_cli = find_program('appstreamcli', required: false)
cargo = find_program('cargo', required: true)

version = meson.project_version()

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
localedir = prefix / get_option('localedir')

datadir = prefix / get_option('datadir')
pkgdatadir = datadir / meson.project_name()
iconsdir = datadir / 'icons'
podir = meson.project_source_root() / 'po'
gettext_package = meson.project_name()

if get_option('profile') == 'development'
  profile = 'Devel'
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD', check: true).stdout().strip()
  if vcs_tag == ''
    version_suffix = '-devel'
  else
    version_suffix = '-@0@'.format(vcs_tag)
  endif
  application_id = '@0@.@1@'.format(base_id, profile)
else
  profile = ''
  version_suffix = ''
  application_id = base_id
endif

meson.add_dist_script(
  'build-aux/dist-vendor.sh',
  meson.project_build_root() / 'meson-dist' / meson.project_name()
  + '-'
  + version,
  meson.project_source_root(),
)

if get_option('profile') != 'release'
  # Setup pre-commit hook for ensuring coding style is always consistent
  message('Setting up git pre-commit hook')
  run_command('cp', '-f', 'hooks/pre-commit.hook', '.git/hooks/pre-commit', check: false)
endif

subdir('data')
subdir('help')
subdir('po')
subdir('src')

gnome.post_install(
  gtk_update_icon_cache: true,
  glib_compile_schemas: false,
  update_desktop_database: true,
)