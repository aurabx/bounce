import type { Metadata } from 'next'
import { Inter } from 'next/font/google'
import React from 'react'
import './globals.css'
import { cn } from "@/app/lib/utils";
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
      <html lang="en" className="h-full">
          <body className={cn('h-full bg-background antialiased', inter.className)}>
              <Providers>
                  <EventHandler>
                      <PageLayout>
                          {children}
                      </PageLayout>
                      <Tray/>
                  </EventHandler>
              </Providers>
          </body>
      </html>
  )
}
