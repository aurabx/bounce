
import type { Metadata } from 'next'
import { Inter } from 'next/font/google'
import './globals.css'
import MenuItem from "@/app/components/MenuItem";
import {classNames} from "@/app/helpers";

const inter = Inter({ subsets: ['latin'] })

export const metadata: Metadata = {
  title: 'Aurabox Bounce',
  description: 'Dicom proxy recaster and packager',
}



export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="en" className="h-full bg-white">
      <body className={classNames('h-full', 'bg-slate-100', 'bg-none', inter.className)}>
        <div>
          <div className="fixed inset-y-0 flex w-48 flex-col bg-white">
            <div className="flex grow flex-col overflow-y-auto bg-indigo-600 px-6 py-4">
              <nav className="flex flex-1 flex-col">
                <ul role="list" className="flex flex-1 flex-col">
                  <li>
                    <ul role="list" className="space-y-2">
                      <MenuItem href="/" label="Dashboard" />
                      <MenuItem href="/settings" label="Settings" />
                    </ul>
                  </li>
                </ul>
              </nav>
            </div>
          </div>
          <div className="pl-48">
            <main className="py-4">
              <div className="p-4  max-w-xl">{children}</div>
            </main>
          </div>
        </div>
      </body>
    </html>
  )
}
