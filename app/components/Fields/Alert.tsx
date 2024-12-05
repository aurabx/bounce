export default function Alert({ children } : { children: any}) {
    return (
        <div className="rounded-md bg-green-50 p-4">
            <div className="mt-2 text-sm text-green-700">
                {children}
            </div>
        </div>
    )
}