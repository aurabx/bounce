
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
      <body className={classNames('h-full', inter.className)}>
        <div>
          <div className="fixed inset-y-0 flex w-72 flex-col">
            <div className="flex grow flex-col overflow-y-auto bg-indigo-600 px-6 py-4">
              <nav className="flex flex-1 flex-col">
                <ul role="list" className="flex flex-1 flex-col">
                  <li>
                    <ul role="list">
                      <MenuItem href="/" label="Dashboard" />
                      <MenuItem href="/settings" label="Settings" />
                    </ul>
                  </li>
                </ul>
              </nav>
            </div>
          </div>

          <div className="pl-72">
            <main className="py-10">
              <div className="px-4 sm:px-6 lg:px-8">{children}</div>
            </main>
          </div>
        </div>
      </body>
    </html>
  )
}
