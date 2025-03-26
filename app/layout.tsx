import type { Metadata } from 'next'
import { Inter } from 'next/font/google'
import React from 'react'
import './globals.css'
import {classNames} from "@/app/lib/helpers";
import Providers from './components/Providers'
import EventHandler from "@/app/components/EventHandler";
import Tray from "@/app/components/Tray";
import PageLayout from "@/app/components/PageLayout";

const inter = Inter({ subsets: ['latin'] })

export const metadata: Metadata = {
  title: 'Aurabox Bounce',
  description: 'Dicom proxy recaster and packager',
}




export default function RootLayout({ children, }: {
  children: React.ReactNode
}) {


  return (
      <Providers>
          <EventHandler>
              <html lang="en" className="h-full bg-slate-100">
                  <body className={classNames('h-full', 'bg-slate-100', 'bg-none', inter.className)}>
                    <PageLayout>
                        {children}
                    </PageLayout>
                  </body>
              </html>
          </EventHandler>
          <Tray/>
      </Providers>
  )
}
