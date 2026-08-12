'use client'

import { useCallback, useEffect, useState } from 'react'
import { Copy, Maximize2, Minus, X } from 'lucide-react'

import { ModeToggle } from '@/components/mode-toggle'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

interface WindowApi {
  minimize: () => Promise<void>
  maximize: () => Promise<void>
  unmaximize: () => Promise<void>
  close: () => Promise<void>
  isMaximized: () => Promise<boolean>
}

interface TitleBarProps {
  className?: string
}

interface WindowButtonProps {
  onClick: () => void
  title: string
  children: React.ReactNode
  className?: string
}

function AppLogo({ className }: { className?: string }) {
  return (
    <svg
      width='20'
      height='18'
      viewBox='0 0 32 28'
      fill='currentColor'
      className={className}
      xmlns='http://www.w3.org/2000/svg'
    >
      <path d='M15.9484 7.4544C15.6067 7.11269 15.0527 7.11269 14.711 7.4544C14.3692 7.79611 14.3692 8.35013 14.711 8.69184L21.4285 15.4094C21.7702 15.7511 22.3242 15.7511 22.6659 15.4094C23.0076 15.0676 23.0076 14.5136 22.6659 14.1719L15.9484 7.4544Z' />
      <path d='M13.2965 10.1063C12.9548 9.76455 12.4008 9.76455 12.0591 10.1063C11.7174 10.448 11.7174 11.002 12.0591 11.3437L16.125 15.4096C16.4667 15.7513 17.0207 15.7513 17.3624 15.4096C17.7041 15.0678 17.7041 14.5138 17.3624 14.1721L13.2965 10.1063Z' />
      <path d='M10.6449 12.7579C10.3032 12.4162 9.7492 12.4162 9.40749 12.7579C9.06578 13.0996 9.06578 13.6536 9.40749 13.9953L10.8217 15.4095C11.1634 15.7512 11.7174 15.7512 12.0591 15.4095C12.4008 15.0678 12.4008 14.5138 12.0591 14.1721L10.6449 12.7579Z' />
      <path d='M15.9404 3.00002L3.19549 16.2547C3.08282 16.3719 3.01879 16.5048 3.00339 16.6534C2.98992 16.7834 3.01925 16.9005 3.09139 17.0047C4.05599 18.3985 5.00465 19.5103 5.93739 20.3401C6.76785 21.0789 8.02965 21.9871 9.72279 23.0646C10.4884 23.5519 11.3283 23.9204 12.2426 24.1701C13.1349 24.4139 14.0641 24.5358 15.0301 24.5358H16.4929C17.5594 24.5358 18.5812 24.3888 19.5584 24.0947C20.5578 23.7941 21.4671 23.3519 22.2864 22.7682C23.8087 21.6837 24.991 20.7547 25.8332 19.9811C26.7874 19.1047 27.7289 18.0954 28.6576 16.9534C28.7464 16.8442 28.7863 16.7182 28.7774 16.5753C28.7674 16.415 28.7024 16.2725 28.5825 16.1477L15.9404 3.00002ZM13.7779 0.920625C14.9583 -0.306875 16.9225 -0.306875 18.1029 0.920625L30.745 14.0684C31.0517 14.3874 31.2933 14.7478 31.4699 15.1497C31.6442 15.5463 31.7448 15.9592 31.7716 16.3883C31.7989 16.8261 31.7478 17.2526 31.6183 17.6679C31.4836 18.0999 31.2726 18.4927 30.9851 18.8462C29.9647 20.101 28.9239 21.2157 27.8626 22.1905C26.929 23.0481 25.6505 24.0551 24.027 25.2116C22.9393 25.9866 21.7378 26.5719 20.4227 26.9675C19.1636 27.3464 17.8537 27.5358 16.4929 27.5358H15.0301C13.7959 27.5358 12.6032 27.3786 11.4521 27.0641C10.2461 26.7347 9.13272 26.2452 8.11199 25.5955C6.28339 24.4318 4.89385 23.4271 3.94339 22.5815C2.83172 21.5925 1.72542 20.3027 0.624487 18.7119C0.38002 18.3587 0.206387 17.9747 0.103587 17.5601C0.00465342 17.1613 -0.0234132 16.7558 0.0193869 16.3436C0.0612535 15.9408 0.16842 15.5541 0.340887 15.1837C0.514753 14.8105 0.745453 14.4744 1.03299 14.1754L13.7779 0.920625Z' />
    </svg>
  )
}

function detectMacOS() {
  if (typeof window === 'undefined') {
    return false
  }

  const platform = window.navigator.userAgent.toLowerCase()
  return platform.includes('mac')
}

function WindowButton({ onClick, title, children, className }: WindowButtonProps) {
  return (
    <Button
      type='button'
      variant='ghost'
      size='icon'
      className={cn(
        'h-8 w-8 rounded-full border-0 transition-colors hover:bg-muted/80 active:bg-muted focus-visible:ring-0 focus-visible:ring-offset-0',
        className
      )}
      onClick={onClick}
      title={title}
    >
      {children}
    </Button>
  )
}

export function TitleBar({ className }: TitleBarProps) {
  const [isMacOS, setIsMacOS] = useState(false)
  const [isMaximized, setIsMaximized] = useState(false)
  const [windowApi, setWindowApi] = useState<WindowApi | null>(null)

  useEffect(() => {
    setIsMacOS(detectMacOS())
  }, [])

  useEffect(() => {
    let unlisten: (() => void) | undefined

    const initWindow = async () => {
      try {
        if (typeof window === 'undefined') {
          return
        }

        const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
        const webviewWindow = getCurrentWebviewWindow()

        setWindowApi({
          minimize: () => webviewWindow.minimize(),
          maximize: () => webviewWindow.maximize(),
          unmaximize: () => webviewWindow.unmaximize(),
          close: () => webviewWindow.close(),
          isMaximized: () => webviewWindow.isMaximized(),
        })

        setIsMaximized(await webviewWindow.isMaximized())
        unlisten = await webviewWindow.onResized(() => {
          webviewWindow.isMaximized().then(setIsMaximized)
        })
      } catch (err) {
        console.log('Window API not available:', err)
      }
    }

    initWindow()

    return () => {
      if (unlisten) {
        unlisten()
      }
    }
  }, [])

  const handleMinimize = useCallback(async () => {
    if (!windowApi) {
      return
    }

    try {
      await windowApi.minimize()
    } catch (err) {
      console.error('Minimize failed:', err)
    }
  }, [windowApi])

  const handleMaximizeRestore = useCallback(async () => {
    if (!windowApi) {
      return
    }

    try {
      if (isMaximized) {
        await windowApi.unmaximize()
      } else {
        await windowApi.maximize()
      }
    } catch (err) {
      console.error('Maximize/Restore failed:', err)
    }
  }, [isMaximized, windowApi])

  const handleClose = useCallback(async () => {
    if (!windowApi) {
      return
    }

    try {
      await windowApi.close()
    } catch (err) {
      console.error('Close failed:', err)
    }
  }, [windowApi])

  return (
    <div
      className={cn(
        'relative h-10 border-b border-border bg-background/95 backdrop-blur select-none',
        className
      )}
    >
      {isMacOS ? (
        <>
          <div className='absolute inset-0' data-tauri-drag-region />

          <div className='pointer-events-none absolute inset-y-0 left-0 right-0 flex items-center justify-center'>
            <div className='flex items-center gap-2' data-tauri-drag-region>
              <AppLogo className='h-5 w-5 text-foreground' />
              <span className='text-sm font-medium text-foreground/80'>RTLauncher</span>
            </div>
          </div>

          <div className='no-drag absolute inset-y-0 right-0 flex items-center pr-3'>
            <ModeToggle />
          </div>
        </>
      ) : (
        <div className='flex h-full items-center' data-tauri-drag-region>
          <div className='flex h-full items-center gap-2 px-3' data-tauri-drag-region>
            <AppLogo className='h-5 w-5 text-foreground' />
            <span className='text-sm font-medium text-foreground/80'>RTLauncher</span>
          </div>

          <div className='h-full flex-1' data-tauri-drag-region />

          <div className='no-drag flex h-full items-center gap-1 pr-3'>
            <ModeToggle />

            <div className='flex items-center gap-1'>
              <WindowButton onClick={handleMinimize} title='最小化'>
                <Minus className='size-4' />
              </WindowButton>

              <WindowButton onClick={handleMaximizeRestore} title={isMaximized ? '还原' : '最大化'}>
                {isMaximized ? <Copy className='size-3.5 rotate-90' /> : <Maximize2 className='size-3.5' />}
              </WindowButton>

              <WindowButton
                onClick={handleClose}
                title='关闭'
                className='hover:bg-destructive hover:text-white active:bg-destructive/90 dark:hover:bg-red-600 dark:active:bg-red-700'
              >
                <X className='size-4' />
              </WindowButton>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
