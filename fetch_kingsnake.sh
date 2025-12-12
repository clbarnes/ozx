#!/bin/sh
DIR="fixtures"
if test -d "$DIR/kingsnake"; then
    exit
fi
if test ! -f "$DIR/kingsnake.ozx"; then
    wget -P $DIR/ https://ome-zarr-scivis.s3.us-east-1.amazonaws.com/v0.5/96x2-ozx/kingsnake.ozx
fi
unzip fixtures/kingsnake.ozx -d $DIR/kingsnake
rm -f $DIR/kingsnake.ozx
