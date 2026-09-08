import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createBrowserRouter, RouterProvider } from 'react-router'
import './index.css'
import { Dashboard } from '@/routes/dashboard/Dashboard'
import { DoctorPage } from '@/routes/doctor/Doctor'
import { ProviderPage } from '@/routes/provider/ProviderPage'
import { SettingsPage } from '@/routes/settings/Settings'
import { Layout } from '@/shell/Layout'

const queryClient = new QueryClient({
    defaultOptions: { queries: { staleTime: 5_000, refetchOnWindowFocus: true, retry: 1 } },
})

const router = createBrowserRouter([
    {
        element: <Layout />,
        children: [
            { path: '/', element: <Dashboard /> },
            { path: '/providers/:id', element: <ProviderPage /> },
            { path: '/settings', element: <SettingsPage /> },
            { path: '/doctor', element: <DoctorPage /> },
        ],
    },
])

createRoot(document.getElementById('root') as HTMLElement).render(
    <StrictMode>
        <QueryClientProvider client={queryClient}>
            <RouterProvider router={router} />
        </QueryClientProvider>
    </StrictMode>
)
