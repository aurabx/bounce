import type { Metadata } from 'next'
import { Inter } from 'next/font/google'
import React from 'react'
import './globals.css'
import MenuItem from "@/app/components/MenuItem";
import {classNames} from "@/app/lib/helpers";
import Providers from './components/Providers'
import EventHandler from "@/app/components/EventHandler";
import Tray from "@/app/components/Tray";
import CurrentStatus from "@/app/components/CurrentStatus";
import PageTitle from "@/app/components/PageTitle";
import {items} from "@/app/lib/menu";

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
    <Providers>
      <EventHandler>
        <html lang="en" className="h-full bg-white">
        <body className={classNames('h-full', 'bg-slate-100', 'bg-none', inter.className)}>
        <div className="h-full">
          <div className="fixed inset-y-0 flex w-48 flex-col bg-white">
            <div className="flex grow flex-col overflow-y-auto bg-indigo-600 px-6 py-4">
              <nav className="flex flex-1 flex-col">
                <ul role="list" className="flex flex-col">
                  <li>
                    <ul role="list" className="space-y-2">
                      {items.map(
                          (item) => <MenuItem key={item.label} {...item}/>
                      )}
                    </ul>
                  </li>
                </ul>
                <CurrentStatus />
              </nav>

            </div>
          </div>
          <div className="pl-48 h-full">
            <PageTitle />
            <main className="py-4 h-full">
              <div className="p-4 h-full">{children}</div>
            </main>
          </div>
        </div>
        </body>
        </html>
      </EventHandler>
      <Tray />
    </Providers>
  )
}
