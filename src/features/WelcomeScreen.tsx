import { open, save } from '@tauri-apps/plugin-dialog'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  createProject,
  getAppSettings,
  openProject,
} from '../ipc'
import { useStore } from '../store'
import { PRODUCT_NAME } from '../brand'
import { AppIcon } from '../components/AppIcon'

/**
 * Shown when no project is currently open (fresh install, or the user
 * chose Project → Close). Offers New / Open buttons plus a recent
 * projects list.
 */
export function WelcomeScreen() {
  const setProject = useStore((s) => s.setProject)
  const qc = useQueryClient()

  const settings = useQuery({
    queryKey: ['app-settings'],
    queryFn: getAppSettings,
  })
  const recents = settings.data?.recentProjects ?? []

  const newProject = useMutation({
    mutationFn: async () => {
      const picked = await save({
        title: `Create a new ${PRODUCT_NAME} project`,
        defaultPath: 'MyProject.magpie',
        filters: [{ name: `${PRODUCT_NAME} project`, extensions: ['magpie'] }],
      })
      if (!picked) return null
      return createProject(picked)
    },
    onSuccess: (info) => {
      if (!info) return
      setProject(info)
      qc.invalidateQueries({ queryKey: ['app-settings'] })
      qc.invalidateQueries({ queryKey: ['folders'] })
      qc.invalidateQueries({ queryKey: ['images'] })
      qc.invalidateQueries({ queryKey: ['tags'] })
    },
    onError: (err) => {
      alert(`Could not create project: ${(err as Error).message}`)
    },
  })

  const openExisting = useMutation({
    mutationFn: async (path?: string) => {
      let p = path
      if (!p) {
        const picked = await open({
          multiple: false,
          title: `Open a ${PRODUCT_NAME} project`,
          filters: [{ name: `${PRODUCT_NAME} project`, extensions: ['magpie', 'db'] }],
        })
        if (!picked) return null
        p = typeof picked === 'string' ? picked : (picked as string)
      }
      return openProject(p!)
    },
    onSuccess: (info) => {
      if (!info) return
      setProject(info)
      qc.invalidateQueries({ queryKey: ['app-settings'] })
      qc.invalidateQueries({ queryKey: ['folders'] })
      qc.invalidateQueries({ queryKey: ['images'] })
      qc.invalidateQueries({ queryKey: ['tags'] })
    },
    onError: (err) => {
      alert(`Could not open project: ${(err as Error).message}`)
    },
  })

  return (
    <div className="h-full grid place-items-center p-8">
      <div className="w-[480px] max-w-full text-center">
        <div className="flex items-center justify-center gap-3 mb-6">
          <AppIcon className="w-10 h-10 rounded" />
          <div className="text-2xl font-semibold tracking-tight">{PRODUCT_NAME}</div>
        </div>
        <p className="text-slate-400 text-sm mb-8">
          Create a new project to start organising your files, or open one
          you already have.
        </p>

        <div className="flex flex-col gap-3">
          <button
            className="btn-primary justify-center py-2.5"
            onClick={() => newProject.mutate()}
            disabled={newProject.isPending}
          >
            {newProject.isPending ? 'Creating…' : 'New Project…'}
          </button>
          <button
            className="btn justify-center py-2.5"
            onClick={() => openExisting.mutate(undefined)}
            disabled={openExisting.isPending}
          >
            {openExisting.isPending ? 'Opening…' : 'Open Project…'}
          </button>
        </div>

        {recents.length > 0 && (
          <div className="mt-8 text-left">
            <div className="text-[11px] uppercase tracking-wider text-slate-500 mb-2">
              Recent projects
            </div>
            <ul className="flex flex-col gap-1">
              {recents.map((p) => (
                <li key={p}>
                  <button
                    className="w-full text-left text-sm text-slate-300 hover:text-white hover:bg-surface-hover rounded px-2 py-1.5 truncate"
                    title={p}
                    onClick={() => openExisting.mutate(p)}
                    disabled={openExisting.isPending}
                  >
                    {shortName(p)}
                    <span className="text-slate-500 text-[11px] ml-2 truncate">
                      {parentDir(p)}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </div>
  )
}

function shortName(p: string): string {
  const norm = p.replace(/\\/g, '/')
  const parts = norm.split('/').filter(Boolean)
  return parts.at(-1) ?? p
}

function parentDir(p: string): string {
  const norm = p.replace(/\\/g, '/')
  const parts = norm.split('/').filter(Boolean)
  parts.pop()
  return parts.join('/')
}
